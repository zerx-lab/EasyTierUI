//! Persistence: profiles as JSON in the app's key-value storage (`profiles.v1`), secrets in the keychain, and the
//! installation's machine id.
//!
//! The JSON holds no secret and is schema-versioned. Loading is tolerant: unknown fields are ignored, an entry that
//! cannot be read is skipped with a log line (and left untouched in storage, so a newer version can still read it),
//! and a list written by a newer schema is refused rather than rewritten. Storage and keychain calls are quick but
//! may block (a keychain prompt): the store runs the `*_secret*` and `delete_profile` functions through
//! `core::work::off_thread`, and the key-value ones on the engine thread, where NSH's `Storage` lives.

use std::{
    collections::BTreeMap,
    sync::{Mutex, PoisonError},
};

use serde::Serialize;
use serde_json::Value;

use super::{
    error::ProfileError,
    model::{NetworkProfile, ProfileId, random_uuid},
};
use crate::core::{
    acl::GroupSecrets,
    keychain::{KeychainError, SecretStore},
    secret::Secret,
};

/// The storage key of the profile list.
pub const PROFILES_KEY: &str = "profiles.v1";
/// The storage key of the installation's machine id.
pub const MACHINE_ID_KEY: &str = "node.machine-id";
/// The schema version this app writes.
pub const SCHEMA_VERSION: u32 = 1;

/// Why persistence failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RepoError {
    /// The stored list is not the expected JSON; it is left as it is.
    #[error("the stored profile list is damaged")]
    Damaged,
    /// The list was written by a newer version of the app; saving would lose its newer fields.
    #[error("the stored profiles were written by a newer version of the app (schema {found})")]
    NewerSchema {
        /// The schema version found.
        found: u32,
    },
    /// The app's storage quota is used up.
    #[error("the app's storage is full")]
    StorageFull,
    /// The keychain refused.
    #[error(transparent)]
    Keychain(#[from] KeychainError),
    /// The profile is not valid, or no id could be generated.
    #[error(transparent)]
    Profile(#[from] ProfileError),
    /// The profile could not be written as JSON (a bug).
    #[error("the profile could not be written")]
    Serialize,
    /// The stored ACL group secrets are not what this app wrote.
    #[error("the stored ACL group secrets could not be read")]
    GroupSecrets,
}

/// The key-value storage the profile list lives in: NSH's [`NshStorage`] in the app, [`MemoryKv`] in tests.
pub trait KeyValue {
    /// The value under `key`.
    fn get(&self, key: &str) -> Option<String>;

    /// Stores `value` under `key`.
    ///
    /// # Errors
    ///
    /// [`RepoError::StorageFull`] when the quota would be exceeded.
    fn set(&self, key: &str, value: String) -> Result<(), RepoError>;

    /// Removes `key`.
    fn remove(&self, key: &str);
}

/// NSH's `Storage`: the app's persistent key-value store (a file on the desktops and mobile, `localStorage` on the
/// web). Plaintext: nothing secret goes in. Call it where components run (the engine thread).
#[derive(Debug, Clone, Copy, Default)]
pub struct NshStorage;

impl KeyValue for NshStorage {
    fn get(&self, key: &str) -> Option<String> {
        nsh_core::storage::Storage::get_item(key)
    }

    fn set(&self, key: &str, value: String) -> Result<(), RepoError> {
        nsh_core::storage::Storage::set_item(key, value).map_err(|_| RepoError::StorageFull)
    }

    fn remove(&self, key: &str) {
        nsh_core::storage::Storage::remove_item(key);
    }
}

/// Key-value storage in memory, for tests and for code that runs without an engine.
#[derive(Debug, Default)]
pub struct MemoryKv(Mutex<BTreeMap<String, String>>);

impl KeyValue for MemoryKv {
    fn get(&self, key: &str) -> Option<String> {
        let entries = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        entries.get(key).cloned()
    }

    fn set(&self, key: &str, value: String) -> Result<(), RepoError> {
        let mut entries = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        entries.insert(key.to_owned(), value);
        Ok(())
    }

    fn remove(&self, key: &str) {
        let mut entries = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        entries.remove(key);
    }
}

/// What [`load_profiles`] found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LoadedProfiles {
    /// The profiles that could be read, in stored order.
    pub profiles: Vec<NetworkProfile>,
    /// How many entries could not be read (and were left in storage): for a notice in the UI.
    pub skipped: usize,
}

#[derive(Serialize)]
struct StoredList<'a> {
    version: u32,
    profiles: &'a [Value],
}

/// The stored entries as raw JSON, so entries this version cannot read survive a rewrite.
fn read_entries(kv: &dyn KeyValue) -> Result<Vec<Value>, RepoError> {
    let Some(text) = kv.get(PROFILES_KEY) else {
        return Ok(Vec::new());
    };
    let Ok(Value::Object(mut document)) = serde_json::from_str::<Value>(&text) else {
        log::error!("the stored profile list is not valid JSON");
        return Err(RepoError::Damaged);
    };
    let found = document
        .get("version")
        .and_then(Value::as_u64)
        .map_or(SCHEMA_VERSION, |version| {
            u32::try_from(version).unwrap_or(u32::MAX)
        });
    if found > SCHEMA_VERSION {
        return Err(RepoError::NewerSchema { found });
    }
    match document.remove("profiles") {
        None => Ok(Vec::new()),
        Some(Value::Array(entries)) => Ok(entries),
        Some(_) => Err(RepoError::Damaged),
    }
}

fn write_entries(kv: &dyn KeyValue, entries: &[Value]) -> Result<(), RepoError> {
    let list = StoredList {
        version: SCHEMA_VERSION,
        profiles: entries,
    };
    let text = serde_json::to_string(&list).map_err(|_| RepoError::Serialize)?;
    kv.set(PROFILES_KEY, text)
}

fn entry_id(entry: &Value) -> Option<&str> {
    entry.get("id").and_then(Value::as_str)
}

/// Reads the stored profiles. Entries that cannot be read are skipped, logged and counted in
/// [`LoadedProfiles::skipped`]; a repeated id keeps its first entry.
///
/// # Errors
///
/// [`RepoError::Damaged`] when the stored list is not JSON of the expected shape; [`RepoError::NewerSchema`] when a
/// newer app wrote it.
pub fn load_profiles(kv: &dyn KeyValue) -> Result<LoadedProfiles, RepoError> {
    let mut loaded = LoadedProfiles::default();
    for (index, entry) in read_entries(kv)?.into_iter().enumerate() {
        match serde_json::from_value::<NetworkProfile>(entry) {
            Ok(profile) if loaded.profiles.iter().any(|known| known.id == profile.id) => {
                log::warn!("skipping profile entry {index}: its id repeats an earlier entry");
                loaded.skipped = loaded.skipped.saturating_add(1);
            }
            Ok(profile) => loaded.profiles.push(profile),
            Err(error) => {
                log::warn!("skipping unreadable profile entry {index}: {error}");
                loaded.skipped = loaded.skipped.saturating_add(1);
            }
        }
    }
    Ok(loaded)
}

/// Stores `profile`, replacing the entry with its id or appending a new one. Other entries, readable or not, are
/// kept as they are.
///
/// # Errors
///
/// The first validation problem; [`RepoError::Damaged`], [`RepoError::NewerSchema`], [`RepoError::StorageFull`].
pub fn save_profile(kv: &dyn KeyValue, profile: &NetworkProfile) -> Result<(), RepoError> {
    profile.validate()?;
    let value = serde_json::to_value(profile).map_err(|_| RepoError::Serialize)?;
    let mut entries = read_entries(kv)?;
    match entries
        .iter_mut()
        .find(|entry| entry_id(entry) == Some(profile.id.as_str()))
    {
        Some(existing) => *existing = value,
        None => entries.push(value),
    }
    write_entries(kv, &entries)
}

/// Removes the entry with `id`; `Ok(false)` when there was none.
///
/// # Errors
///
/// [`RepoError::Damaged`], [`RepoError::NewerSchema`], [`RepoError::StorageFull`].
pub fn remove_profile(kv: &dyn KeyValue, id: &ProfileId) -> Result<bool, RepoError> {
    let mut entries = read_entries(kv)?;
    let before = entries.len();
    entries.retain(|entry| entry_id(entry) != Some(id.as_str()));
    if entries.len() == before {
        return Ok(false);
    }
    write_entries(kv, &entries)?;
    Ok(true)
}

/// Keeps `secret` in the keychain under the profile's account: the network secret of a local profile, the
/// control-plane address of a managed one.
///
/// # Errors
///
/// [`RepoError::Keychain`].
pub fn save_secret(
    secrets: &dyn SecretStore,
    profile: &NetworkProfile,
    secret: &Secret,
) -> Result<(), RepoError> {
    Ok(secrets.set(&profile.secret_account(), secret)?)
}

/// The profile's secret from the keychain, if it has one.
///
/// # Errors
///
/// [`RepoError::Keychain`].
pub fn load_secret(
    secrets: &dyn SecretStore,
    profile: &NetworkProfile,
) -> Result<Option<Secret>, RepoError> {
    Ok(secrets.get(&profile.secret_account())?)
}

/// The ACL group secrets of `profile` from the keychain; none when nothing is stored.
///
/// # Errors
///
/// [`RepoError::Keychain`]; [`RepoError::GroupSecrets`] when the stored value is not readable.
pub fn load_group_secrets(
    secrets: &dyn SecretStore,
    profile: &NetworkProfile,
) -> Result<GroupSecrets, RepoError> {
    match secrets.get(&profile.id.acl_group_secrets_account())? {
        Some(value) => GroupSecrets::from_keychain(&value).map_err(|_| RepoError::GroupSecrets),
        None => Ok(GroupSecrets::default()),
    }
}

/// Keeps the ACL group secrets of `profile` in the keychain as one entry; an empty set deletes the entry.
///
/// # Errors
///
/// [`RepoError::Keychain`]; [`RepoError::GroupSecrets`] if the secrets cannot be encoded.
pub fn save_group_secrets(
    secrets: &dyn SecretStore,
    profile: &NetworkProfile,
    groups: &GroupSecrets,
) -> Result<(), RepoError> {
    let account = profile.id.acl_group_secrets_account();
    if groups.is_empty() {
        return Ok(secrets.delete(&account)?);
    }
    let value = groups.to_keychain().map_err(|_| RepoError::GroupSecrets)?;
    Ok(secrets.set(&account, &value)?)
}

/// Stores a profile and its secret: the secret first (a profile without its secret is worse than a secret without
/// a profile), then the entry. Without a `secret` the keychain is left alone.
///
/// # Errors
///
/// As [`save_profile`] and [`save_secret`].
pub fn save(
    kv: &dyn KeyValue,
    secrets: &dyn SecretStore,
    profile: &NetworkProfile,
    secret: Option<&Secret>,
) -> Result<(), RepoError> {
    profile.validate()?;
    if let Some(secret) = secret {
        save_secret(secrets, profile, secret)?;
    }
    save_profile(kv, profile)
}

/// Deletes a profile: its keychain entries (the network secret, the control-plane address and the ACL group
/// secrets), then its stored entry. If the keychain refuses, the profile stays so the deletion can be retried.
///
/// # Errors
///
/// [`RepoError::Keychain`], then as [`remove_profile`].
pub fn delete_profile(
    kv: &dyn KeyValue,
    secrets: &dyn SecretStore,
    id: &ProfileId,
) -> Result<(), RepoError> {
    secrets.delete(&id.network_secret_account())?;
    secrets.delete(&id.config_server_account())?;
    secrets.delete(&id.acl_group_secrets_account())?;
    remove_profile(kv, id)?;
    Ok(())
}

/// This installation's machine id (a UUID in hyphenated lowercase text): generated once, then the same on every
/// call. A control plane knows this device by it, so it must not change; a stored value that is not a UUID is
/// replaced.
///
/// # Errors
///
/// [`ProfileError::NoRandomSource`] (as [`RepoError::Profile`]) if one must be generated and there is no random
/// source; [`RepoError::StorageFull`].
pub fn machine_id(kv: &dyn KeyValue) -> Result<String, RepoError> {
    if let Some(stored) = kv.get(MACHINE_ID_KEY) {
        if stored.len() == 36 && uuid::Uuid::try_parse(&stored).is_ok() {
            return Ok(stored);
        }
        log::warn!("the stored machine id is not a UUID; generating a new one");
    }
    let fresh = random_uuid()?.hyphenated().to_string();
    kv.set(MACHINE_ID_KEY, fresh.clone())?;
    Ok(fresh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::keychain::MemoryStore,
        profile::model::{IpMode, Socks5},
    };

    fn id(n: char) -> ProfileId {
        ProfileId::parse(&n.to_string().repeat(32)).unwrap()
    }

    fn profile(n: char) -> NetworkProfile {
        let mut p = NetworkProfile::new(id(n), format!("Net {n}"), format!("net-{n}"));
        p.ip = IpMode::Static("10.144.144.1/24".parse().unwrap());
        p.peers = vec!["tcp://203.0.113.7:11010".parse().unwrap()];
        p.socks5 = Some(Socks5::local(1080));
        p
    }

    fn secret() -> Secret {
        Secret::new("example-secret-do-not-use")
    }

    #[test]
    fn an_empty_store_loads_nothing() {
        let kv = MemoryKv::default();
        assert_eq!(load_profiles(&kv).unwrap(), LoadedProfiles::default());
    }

    #[test]
    fn profiles_save_load_replace_and_remove() {
        let kv = MemoryKv::default();
        save_profile(&kv, &profile('a')).unwrap();
        save_profile(&kv, &profile('b')).unwrap();
        let loaded = load_profiles(&kv).unwrap();
        assert_eq!(loaded.profiles, vec![profile('a'), profile('b')]);
        assert_eq!(loaded.skipped, 0);

        let mut changed = profile('a');
        changed.name = "Renamed".to_owned();
        save_profile(&kv, &changed).unwrap();
        let loaded = load_profiles(&kv).unwrap();
        assert_eq!(loaded.profiles, vec![changed, profile('b')]);

        assert!(remove_profile(&kv, &id('a')).unwrap());
        assert!(!remove_profile(&kv, &id('a')).unwrap());
        assert_eq!(load_profiles(&kv).unwrap().profiles, vec![profile('b')]);
    }

    #[test]
    fn invalid_profiles_are_not_saved() {
        let kv = MemoryKv::default();
        let mut bad = profile('a');
        bad.network_name = String::new();
        assert!(matches!(
            save_profile(&kv, &bad),
            Err(RepoError::Profile(ProfileError::Empty { .. }))
        ));
        assert_eq!(kv.get(PROFILES_KEY), None);
    }

    #[test]
    fn the_stored_json_is_versioned_and_holds_no_secret() {
        let kv = MemoryKv::default();
        let secrets = MemoryStore::default();
        save(&kv, &secrets, &profile('a'), Some(&secret())).unwrap();
        let text = kv.get(PROFILES_KEY).unwrap();
        assert!(text.starts_with("{\"version\":1,\"profiles\":["));
        assert!(!text.contains("example-secret"));
    }

    #[test]
    fn unreadable_entries_are_skipped_but_kept() {
        let kv = MemoryKv::default();
        let good = serde_json::to_value(profile('a')).unwrap();
        let from_the_future = serde_json::json!({"id": "b".repeat(32), "name": "Future", "network_name": "n", "encryption": "quantum"});
        let nonsense = serde_json::json!(42);
        let repeated = good.clone();
        let list = serde_json::json!({"version": 1, "profiles": [good, from_the_future, nonsense, repeated]});
        kv.set(PROFILES_KEY, list.to_string()).unwrap();

        let loaded = load_profiles(&kv).unwrap();
        assert_eq!(loaded.profiles, vec![profile('a')]);
        assert_eq!(loaded.skipped, 3);

        // Saving another profile does not drop what could not be read.
        save_profile(&kv, &profile('c')).unwrap();
        let text = kv.get(PROFILES_KEY).unwrap();
        assert!(text.contains("quantum"));
        assert_eq!(load_profiles(&kv).unwrap().profiles.len(), 2);
    }

    #[test]
    fn unknown_fields_are_tolerated() {
        let kv = MemoryKv::default();
        let mut entry = serde_json::to_value(profile('a')).unwrap();
        entry["added_in_v2"] = serde_json::json!({"x": 1});
        let list = serde_json::json!({"version": 1, "profiles": [entry], "also_new": true});
        kv.set(PROFILES_KEY, list.to_string()).unwrap();
        assert_eq!(load_profiles(&kv).unwrap().profiles, vec![profile('a')]);
    }

    #[test]
    fn damaged_and_newer_lists_are_refused_not_overwritten() {
        let kv = MemoryKv::default();
        kv.set(PROFILES_KEY, "{not json".to_owned()).unwrap();
        assert_eq!(load_profiles(&kv), Err(RepoError::Damaged));
        assert_eq!(save_profile(&kv, &profile('a')), Err(RepoError::Damaged));
        assert_eq!(kv.get(PROFILES_KEY).as_deref(), Some("{not json"));

        kv.set(PROFILES_KEY, "{\"version\":2,\"profiles\":[]}".to_owned())
            .unwrap();
        assert_eq!(load_profiles(&kv), Err(RepoError::NewerSchema { found: 2 }));
        assert_eq!(
            save_profile(&kv, &profile('a')),
            Err(RepoError::NewerSchema { found: 2 })
        );

        kv.set(PROFILES_KEY, "{\"version\":1,\"profiles\":{}}".to_owned())
            .unwrap();
        assert_eq!(load_profiles(&kv), Err(RepoError::Damaged));
        kv.set(PROFILES_KEY, "[]".to_owned()).unwrap();
        assert_eq!(load_profiles(&kv), Err(RepoError::Damaged));
    }

    #[test]
    fn secrets_rest_in_the_keychain_under_the_profile_accounts() {
        let kv = MemoryKv::default();
        let secrets = MemoryStore::default();
        let p = profile('a');
        save(&kv, &secrets, &p, Some(&secret())).unwrap();
        assert_eq!(
            secrets
                .get(&format!("profile/{}/network-secret", "a".repeat(32)))
                .unwrap(),
            Some(secret())
        );
        assert_eq!(load_secret(&secrets, &p).unwrap(), Some(secret()));
        // Saving without a secret leaves the keychain alone.
        save(&kv, &secrets, &p, None).unwrap();
        assert_eq!(load_secret(&secrets, &p).unwrap(), Some(secret()));
    }

    #[test]
    fn managed_profiles_keep_their_enrollment_under_the_config_server_account() {
        let kv = MemoryKv::default();
        let secrets = MemoryStore::default();
        let p = NetworkProfile::managed(id('d'), "Office");
        let address = Secret::new("udp://control.example.com:22020/team-7f3k9q");
        save(&kv, &secrets, &p, Some(&address)).unwrap();
        assert_eq!(
            secrets
                .get(&format!("profile/{}/config-server", "d".repeat(32)))
                .unwrap(),
            Some(address.clone())
        );
        assert_eq!(load_secret(&secrets, &p).unwrap(), Some(address));
        assert!(!kv.get(PROFILES_KEY).unwrap().contains("team-7f3k9q"));
    }

    #[test]
    fn deleting_removes_the_entry_and_both_secrets() {
        let kv = MemoryKv::default();
        let secrets = MemoryStore::default();
        save(&kv, &secrets, &profile('a'), Some(&secret())).unwrap();
        save(&kv, &secrets, &profile('b'), Some(&secret())).unwrap();
        secrets
            .set(
                &id('a').config_server_account(),
                &Secret::new("udp://x.example.com/team"),
            )
            .unwrap();

        delete_profile(&kv, &secrets, &id('a')).unwrap();
        assert_eq!(load_profiles(&kv).unwrap().profiles, vec![profile('b')]);
        assert_eq!(
            secrets.get(&id('a').network_secret_account()).unwrap(),
            None
        );
        assert_eq!(secrets.get(&id('a').config_server_account()).unwrap(), None);
        assert_eq!(
            secrets.get(&id('b').network_secret_account()).unwrap(),
            Some(secret())
        );
        // Deleting what is not there succeeds.
        delete_profile(&kv, &secrets, &id('a')).unwrap();
    }

    #[test]
    fn a_failing_keychain_keeps_the_profile() {
        struct Locked;
        impl SecretStore for Locked {
            fn get(&self, _: &str) -> Result<Option<Secret>, KeychainError> {
                Err(KeychainError::Platform("locked".into()))
            }
            fn set(&self, _: &str, _: &Secret) -> Result<(), KeychainError> {
                Err(KeychainError::Platform("locked".into()))
            }
            fn delete(&self, _: &str) -> Result<(), KeychainError> {
                Err(KeychainError::Platform("locked".into()))
            }
            fn persistent(&self) -> bool {
                true
            }
        }
        let kv = MemoryKv::default();
        save_profile(&kv, &profile('a')).unwrap();
        assert!(matches!(
            delete_profile(&kv, &Locked, &id('a')),
            Err(RepoError::Keychain(_))
        ));
        assert_eq!(load_profiles(&kv).unwrap().profiles.len(), 1);
        // A profile whose secret cannot be saved is not saved either.
        assert!(matches!(
            save(&kv, &Locked, &profile('b'), Some(&secret())),
            Err(RepoError::Keychain(_))
        ));
        assert_eq!(load_profiles(&kv).unwrap().profiles.len(), 1);
    }

    #[test]
    fn profiles_stored_before_the_flags_existed_still_load() {
        let kv = MemoryKv::default();
        let old = serde_json::json!({
            "id": "a".repeat(32),
            "name": "Old",
            "network_name": "old-net",
            "latency_first": true,
            "accept_dns": true,
            "subnet_proxy": ["192.168.50.0/24"],
            "multi_thread": true,
        });
        let list = serde_json::json!({"version": SCHEMA_VERSION, "profiles": [old]});
        kv.set(PROFILES_KEY, list.to_string()).unwrap();
        let loaded = load_profiles(&kv).unwrap();
        assert_eq!(loaded.skipped, 0);
        let p = loaded.profiles.first().unwrap();
        assert!(p.flags.latency_first && p.flags.accept_dns);
        assert!(p.flags.bind_device && p.flags.multi_thread);
        assert!(!p.flags.disable_p2p && !p.flags.proxy_forward_by_system);
        assert_eq!(
            p.subnet_proxy,
            vec![crate::profile::model::SubnetProxy::plain(
                "192.168.50.0/24".parse().unwrap()
            )]
        );
        assert_eq!(p.manual_routes, None);
        assert_eq!(p.vpn_portal, None);
        assert_eq!(p.acl, None);
        assert!(p.mapped_listeners.is_empty());
    }

    #[test]
    fn group_secrets_rest_in_one_keychain_entry_and_leave_with_the_profile() {
        let kv = MemoryKv::default();
        let secrets = MemoryStore::default();
        let (p, groups) = crate::profile::testing::full_profile().unwrap();
        assert_eq!(
            load_group_secrets(&secrets, &p).unwrap(),
            GroupSecrets::default()
        );

        save(&kv, &secrets, &p, Some(&secret())).unwrap();
        save_group_secrets(&secrets, &p, &groups).unwrap();
        assert_eq!(load_group_secrets(&secrets, &p).unwrap(), groups);
        assert_eq!(
            p.id.acl_group_secrets_account(),
            format!("profile/{}/acl-group-secrets", p.id.as_str())
        );
        // The stored profile list holds the declared group names, never their secrets.
        let stored = kv.get(PROFILES_KEY).unwrap();
        assert!(stored.contains("admins"));
        assert!(!stored.contains("example-group-secret"));

        // An empty set deletes the entry.
        save_group_secrets(&secrets, &p, &GroupSecrets::default()).unwrap();
        assert_eq!(
            secrets.get(&p.id.acl_group_secrets_account()).unwrap(),
            None
        );

        save_group_secrets(&secrets, &p, &groups).unwrap();
        delete_profile(&kv, &secrets, &p.id).unwrap();
        assert_eq!(
            secrets.get(&p.id.acl_group_secrets_account()).unwrap(),
            None
        );
        assert_eq!(secrets.get(&p.id.network_secret_account()).unwrap(), None);
    }

    #[test]
    fn an_unreadable_group_secrets_entry_is_reported() {
        let secrets = MemoryStore::default();
        let p = profile('a');
        secrets
            .set(&p.id.acl_group_secrets_account(), &Secret::new("not json"))
            .unwrap();
        assert_eq!(
            load_group_secrets(&secrets, &p),
            Err(RepoError::GroupSecrets)
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_machine_id_is_generated_once_and_stays() {
        let kv = MemoryKv::default();
        let first = machine_id(&kv).unwrap();
        assert_eq!(first.len(), 36);
        assert!(uuid::Uuid::try_parse(&first).is_ok());
        assert_eq!(first, first.to_lowercase());
        assert_eq!(machine_id(&kv).unwrap(), first);
        assert_eq!(kv.get(MACHINE_ID_KEY).as_deref(), Some(first.as_str()));

        kv.set(MACHINE_ID_KEY, "not-a-uuid".to_owned()).unwrap();
        let replaced = machine_id(&kv).unwrap();
        assert_ne!(replaced, "not-a-uuid");
        assert_eq!(machine_id(&kv).unwrap(), replaced);
    }
}

//! EasyTier's GitHub releases: which zip fits this device, the release list, the download and its sha256 check.
//!
//! Networking goes through NSH's `fetch` (the one stack every platform shares), so [`list_releases`] and
//! [`download`] must be awaited in an NSH task. Everything else here is pure and tested against a fixture.

use nsh_core::prelude::{HttpRequest, fetch};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};

use super::error::InstallError;
use crate::core::version::CoreVersion;

/// The release list: newest first, drafts included only for authenticated callers (we are anonymous).
const RELEASES_URL: &str = "https://api.github.com/repos/EasyTier/EasyTier/releases?per_page=15";
/// The largest zip the app downloads into memory (the real ones are 20 to 35 MB).
const MAX_ASSET_BYTES: u64 = 200 * 1024 * 1024;

/// Which of EasyTier's per-platform zips fits a device: the `<os>-<arch>` part of
/// `easytier-<os>-<arch>-v<version>.zip`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssetTarget {
    os: &'static str,
    arch: &'static str,
}

impl AssetTarget {
    /// The zip for the platform this build runs on; `None` where EasyTier publishes none.
    pub fn current() -> Option<Self> {
        Self::for_platform(std::env::consts::OS, std::env::consts::ARCH)
    }

    /// The zip for Rust's `std::env::consts::{OS, ARCH}` names (`macos`, `windows`, `linux` × `x86_64`, `aarch64`,
    /// `x86`, `arm`, `riscv64`, `loongarch64`); `None` for a combination EasyTier publishes no zip for.
    pub fn for_platform(os: &str, arch: &str) -> Option<Self> {
        let (os, arch) = match (os, arch) {
            ("macos", "aarch64") => ("macos", "aarch64"),
            ("macos", "x86_64") => ("macos", "x86_64"),
            ("windows", "x86_64") => ("windows", "x86_64"),
            // Windows calls ARM64 `arm64` in EasyTier's asset names, unlike the other platforms.
            ("windows", "aarch64") => ("windows", "arm64"),
            ("windows", "x86") => ("windows", "i686"),
            ("linux", "x86_64") => ("linux", "x86_64"),
            ("linux", "aarch64") => ("linux", "aarch64"),
            ("linux", "arm") if cfg!(target_abi = "eabihf") => ("linux", "armv7hf"),
            ("linux", "arm") => ("linux", "armv7"),
            ("linux", "riscv64") => ("linux", "riscv64"),
            ("linux", "loongarch64") => ("linux", "loongarch64"),
            _ => return None,
        };
        Some(Self { os, arch })
    }

    /// The zip's file name for the release tagged `tag` (`v2.6.4`).
    pub fn asset_name(&self, tag: &str) -> String {
        format!("easytier-{}-{}-{tag}.zip", self.os, self.arch)
    }
}

/// The zip of a release for this device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    /// File name, `easytier-<os>-<arch>-v<version>.zip`.
    pub name: String,
    /// Size in bytes as listed.
    pub size: u64,
    /// The lowercase hex sha256 from the API's `digest` field; `None` when the release lists none (then the zip is
    /// never installed).
    pub sha256: Option<String>,
    /// The HTTPS download URL.
    pub url: String,
}

/// One EasyTier release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// The git tag, `v2.6.4`.
    pub tag: String,
    /// Its version.
    pub version: CoreVersion,
    /// When GitHub published it, RFC 3339 (`2026-05-12T14:31:08Z`); empty when unknown.
    pub published_at: String,
    /// GitHub marks it a pre-release.
    pub prerelease: bool,
    /// The zip for this device, if the release has one.
    pub asset: Option<Asset>,
}

impl Release {
    /// Whether the app can install it: a zip for this device, a digest to verify it with, a supported version.
    pub fn installable(&self) -> bool {
        self.version.is_supported()
            && self
                .asset
                .as_ref()
                .is_some_and(|asset| asset.sha256.is_some())
    }
}

#[derive(Deserialize)]
struct RawRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<RawAsset>,
}

#[derive(Deserialize)]
struct RawAsset {
    name: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    digest: Option<String>,
    #[serde(default)]
    browser_download_url: String,
}

/// The hex of a `sha256:<hex>` digest, lowercased; `None` for another algorithm or malformed hex.
fn parse_digest(digest: &str) -> Option<String> {
    let hex = digest.trim().strip_prefix("sha256:")?;
    (hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| hex.to_ascii_lowercase())
}

/// Reads GitHub's release list JSON. Drafts, tags that are not versions and entries that do not parse are
/// skipped (logged), never failing the list; `target` picks each release's asset (`None`: no asset anywhere).
///
/// # Errors
///
/// [`InstallError::BadResponse`] when `json` is not an array.
pub fn parse_releases(
    json: &[u8],
    target: Option<&AssetTarget>,
) -> Result<Vec<Release>, InstallError> {
    let items: Vec<serde_json::Value> = serde_json::from_slice(json)
        .map_err(|error| InstallError::BadResponse(error.to_string()))?;
    let mut releases = Vec::new();
    for item in items {
        let raw: RawRelease = match serde_json::from_value(item) {
            Ok(raw) => raw,
            Err(error) => {
                log::warn!("skipping a release entry that does not parse: {error}");
                continue;
            }
        };
        if raw.draft {
            continue;
        }
        let Ok(version) = raw.tag_name.parse::<CoreVersion>() else {
            log::debug!("skipping release {:?}: not a version", raw.tag_name);
            continue;
        };
        let wanted = target.map(|target| target.asset_name(&raw.tag_name));
        let asset = wanted.and_then(|wanted| {
            raw.assets
                .into_iter()
                .find(|asset| asset.name == wanted)
                // A download over anything but HTTPS is never followed.
                .filter(|asset| asset.browser_download_url.starts_with("https://"))
                .map(|asset| Asset {
                    sha256: asset.digest.as_deref().and_then(parse_digest),
                    name: asset.name,
                    size: asset.size,
                    url: asset.browser_download_url,
                })
        });
        releases.push(Release {
            tag: raw.tag_name,
            version,
            published_at: raw.published_at.unwrap_or_default(),
            prerelease: raw.prerelease,
            asset,
        });
    }
    Ok(releases)
}

/// The status of a GitHub answer as an error.
fn check_status(status: u16) -> Result<(), InstallError> {
    match status {
        200..=299 => Ok(()),
        403 | 429 => Err(InstallError::RateLimited),
        other => Err(InstallError::Http(other)),
    }
}

/// The request headers GitHub requires (a `User-Agent`).
fn github_request(url: &str, accept: &str) -> HttpRequest {
    HttpRequest::get(url)
        .header(
            "User-Agent",
            format!("EasyTierUI/{}", env!("CARGO_PKG_VERSION")),
        )
        .header("Accept", accept)
        .header("X-GitHub-Api-Version", "2022-11-28")
}

/// The latest releases (newest first) with each one's zip for this device.
///
/// Await it in an NSH task (`spawn`).
///
/// # Errors
///
/// [`InstallError::Network`] when GitHub is unreachable, [`InstallError::RateLimited`] / [`InstallError::Http`] for
/// its refusals, [`InstallError::BadResponse`] for an unreadable list.
pub async fn list_releases() -> Result<Vec<Release>, InstallError> {
    let response = fetch(github_request(RELEASES_URL, "application/vnd.github+json"))
        .await
        .map_err(InstallError::Network)?;
    check_status(response.status())?;
    parse_releases(&response.body, AssetTarget::current().as_ref())
}

/// Downloads `asset` whole into memory and checks its length against the listing. The digest is checked by the
/// caller ([`verify_sha256`]) before anything is unpacked.
///
/// Await it in an NSH task (`spawn`); `fetch` has no streaming, so there is no byte progress.
///
/// # Errors
///
/// [`InstallError::TooLarge`] before downloading when the listed size exceeds the cap, [`InstallError::Network`] /
/// [`InstallError::Http`] / [`InstallError::RateLimited`] for transport failures, [`InstallError::SizeMismatch`].
pub async fn download(asset: &Asset) -> Result<Vec<u8>, InstallError> {
    if asset.size > MAX_ASSET_BYTES {
        return Err(InstallError::TooLarge(asset.size));
    }
    let response = fetch(github_request(&asset.url, "application/octet-stream"))
        .await
        .map_err(InstallError::Network)?;
    check_status(response.status())?;
    let actual = u64::try_from(response.body.len()).unwrap_or(u64::MAX);
    if actual != asset.size {
        return Err(InstallError::SizeMismatch {
            expected: asset.size,
            actual,
        });
    }
    Ok(response.body)
}

/// The lowercase hex sha256 of `bytes`.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Checks `bytes` against a release's sha256 (hex, any case).
///
/// # Errors
///
/// [`InstallError::DigestMismatch`].
pub fn verify_sha256(bytes: &[u8], expected_hex: &str) -> Result<(), InstallError> {
    if sha256_hex(bytes).eq_ignore_ascii_case(expected_hex.trim()) {
        Ok(())
    } else {
        Err(InstallError::DigestMismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &[u8] = include_bytes!("../../../tests/fixtures/github/releases.json");

    #[test]
    fn asset_names_follow_the_platform() {
        let name = |os, arch| AssetTarget::for_platform(os, arch).map(|t| t.asset_name("v2.6.4"));
        assert_eq!(
            name("macos", "aarch64").as_deref(),
            Some("easytier-macos-aarch64-v2.6.4.zip")
        );
        assert_eq!(
            name("macos", "x86_64").as_deref(),
            Some("easytier-macos-x86_64-v2.6.4.zip")
        );
        assert_eq!(
            name("windows", "x86_64").as_deref(),
            Some("easytier-windows-x86_64-v2.6.4.zip")
        );
        assert_eq!(
            name("windows", "aarch64").as_deref(),
            Some("easytier-windows-arm64-v2.6.4.zip")
        );
        assert_eq!(
            name("windows", "x86").as_deref(),
            Some("easytier-windows-i686-v2.6.4.zip")
        );
        assert_eq!(
            name("linux", "x86_64").as_deref(),
            Some("easytier-linux-x86_64-v2.6.4.zip")
        );
        assert_eq!(
            name("linux", "aarch64").as_deref(),
            Some("easytier-linux-aarch64-v2.6.4.zip")
        );
        assert!(name("linux", "arm").is_some());
        assert_eq!(name("linux", "s390x"), None);
        assert_eq!(name("macos", "x86"), None);
        assert_eq!(name("freebsd", "x86_64"), None);
        assert_eq!(name("ios", "aarch64"), None);
    }

    #[test]
    fn reads_the_release_list() {
        let target = AssetTarget::for_platform("macos", "aarch64").unwrap();
        let releases = parse_releases(FIXTURE, Some(&target)).unwrap();
        // The draft and the non-version tag are gone.
        let tags: Vec<_> = releases.iter().map(|r| r.tag.as_str()).collect();
        assert_eq!(tags, ["v2.6.4", "v2.7.0-rc1", "v2.5.0"]);

        let latest = &releases[0];
        assert_eq!(latest.version.number(), "2.6.4");
        assert_eq!(latest.published_at, "2026-05-12T14:31:08Z");
        assert!(!latest.prerelease);
        let asset = latest.asset.as_ref().unwrap();
        assert_eq!(asset.name, "easytier-macos-aarch64-v2.6.4.zip");
        assert_eq!(asset.size, 30_174_017);
        assert_eq!(
            asset.sha256.as_deref(),
            Some("4be1882d1aa36d31c1d6ba0596f2cf8a097e371f8da124212324b2e0f8df7e4b")
        );
        assert!(
            asset
                .url
                .starts_with("https://github.com/EasyTier/EasyTier/")
        );
        assert!(latest.installable());

        let rc = &releases[1];
        assert!(rc.prerelease);
        assert!(rc.installable());

        // 2.5.0: no macOS zip in the fixture, and too old anyway.
        assert!(releases[2].asset.is_none());
        assert!(!releases[2].installable());
    }

    #[test]
    fn a_zip_without_a_digest_is_never_installable() {
        let target = AssetTarget::for_platform("linux", "x86_64").unwrap();
        let releases = parse_releases(FIXTURE, Some(&target)).unwrap();
        let rc = releases.iter().find(|r| r.tag == "v2.7.0-rc1").unwrap();
        let asset = rc.asset.as_ref().unwrap();
        assert_eq!(asset.sha256, None);
        assert!(!rc.installable());
        // v2.5.0 has no digest field at all and is also too old.
        let old = releases.iter().find(|r| r.tag == "v2.5.0").unwrap();
        assert_eq!(old.asset.as_ref().unwrap().sha256, None);
    }

    #[test]
    fn no_target_means_no_assets_and_garbage_is_an_error() {
        let releases = parse_releases(FIXTURE, None).unwrap();
        assert_eq!(releases.len(), 3);
        assert!(releases.iter().all(|r| r.asset.is_none()));
        assert!(matches!(
            parse_releases(b"{\"message\":\"rate limited\"}", None),
            Err(InstallError::BadResponse(_))
        ));
        assert!(
            parse_releases(b"[{\"tag_name\": 5}, 7]", None)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn digests_are_validated() {
        assert!(parse_digest("sha256:abc").is_none());
        assert!(parse_digest("sha512:".to_owned().as_str()).is_none());
        let upper = format!("sha256:{}", "AB".repeat(32));
        assert_eq!(parse_digest(&upper), Some("ab".repeat(32)));
    }

    #[test]
    fn sha256_accepts_the_match_and_rejects_anything_else() {
        // sha256("abc"), FIPS 180-2's test vector.
        let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(sha256_hex(b"abc"), abc);
        verify_sha256(b"abc", abc).unwrap();
        verify_sha256(b"abc", &abc.to_ascii_uppercase()).unwrap();
        assert_eq!(
            verify_sha256(b"abd", abc),
            Err(InstallError::DigestMismatch)
        );
        assert_eq!(verify_sha256(b"abc", ""), Err(InstallError::DigestMismatch));
    }

    #[test]
    fn statuses_map_to_errors() {
        assert_eq!(check_status(200), Ok(()));
        assert_eq!(check_status(403), Err(InstallError::RateLimited));
        assert_eq!(check_status(404), Err(InstallError::Http(404)));
    }
}

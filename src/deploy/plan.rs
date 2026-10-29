//! The typed deployment plan. Every text value that reaches a template is a validated newtype with a private
//! field, so a renderer cannot be handed an unvalidated string.

use std::fmt;

use url::Url;
use uuid::Uuid;

use super::{
    error::{DeployError, Field, Reason},
    images,
};

fn err(field: Field, reason: Reason) -> DeployError {
    DeployError::new(field, reason)
}

macro_rules! text_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name(String);

        impl $name {
            /// The validated text.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

text_newtype!(
    /// A public DNS name (lower case, at least two labels, no IP address), safe to put in a proxy config.
    Hostname
);

impl Hostname {
    /// Parses `input` as a DNS hostname: 2 to 127 labels of `[a-z0-9-]` (1-63 characters, no leading or trailing
    /// hyphen), at most 253 characters, last label not all digits (that would be an IPv4 address). Upper case is
    /// lowered; internationalized names must be given as punycode.
    ///
    /// # Errors
    ///
    /// [`DeployError`] for [`Field::Domain`] describing the first problem found.
    pub fn parse(input: &str) -> Result<Self, DeployError> {
        Self::parse_as(input, Field::Domain)
    }

    fn parse_as(input: &str, field: Field) -> Result<Self, DeployError> {
        let text = input.trim().to_ascii_lowercase();
        if text.is_empty() {
            return Err(err(field, Reason::Empty));
        }
        if text.len() > 253 {
            return Err(err(field, Reason::TooLong));
        }
        let mut labels = 0_u32;
        let mut last = "";
        for label in text.split('.') {
            if label.is_empty() || label.len() > 63 {
                return Err(err(field, Reason::BadFormat));
            }
            if !label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            {
                return Err(err(field, Reason::BadCharacter));
            }
            if label.starts_with('-') || label.ends_with('-') {
                return Err(err(field, Reason::BadFormat));
            }
            labels = labels.saturating_add(1);
            last = label;
        }
        if labels < 2 || last.bytes().all(|b| b.is_ascii_digit()) {
            return Err(err(field, Reason::BadFormat));
        }
        Ok(Self(text))
    }
}

text_newtype!(
    /// An email address for the ACME account: plain `local@hostname`, no quoting or comments.
    Email
);

impl Email {
    /// Parses `input`.
    ///
    /// # Errors
    ///
    /// [`DeployError`] for [`Field::AcmeEmail`].
    pub fn parse(input: &str) -> Result<Self, DeployError> {
        let text = input.trim();
        let Some((local, host)) = text.split_once('@') else {
            return Err(err(Field::AcmeEmail, Reason::BadFormat));
        };
        if local.is_empty() {
            return Err(err(Field::AcmeEmail, Reason::Empty));
        }
        if local.len() > 64 {
            return Err(err(Field::AcmeEmail, Reason::TooLong));
        }
        let local_ok = local
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'%' | b'+' | b'-'));
        if !local_ok {
            return Err(err(Field::AcmeEmail, Reason::BadCharacter));
        }
        if local.starts_with('.') || local.ends_with('.') || local.contains("..") {
            return Err(err(Field::AcmeEmail, Reason::BadFormat));
        }
        let host = Hostname::parse_as(host, Field::AcmeEmail)?;
        Ok(Self(format!("{local}@{host}")))
    }
}

text_newtype!(
    /// The easytier-web username the server's own node enrolls under: `[a-z0-9-_]{3,32}`, never a seeded or
    /// well-known name.
    OwnerName
);

/// Names an attacker tries first: the seeded accounts and the usual defaults.
const RESERVED_OWNERS: [&str; 10] = [
    "admin",
    "user",
    "root",
    "test",
    "guest",
    "demo",
    "administrator",
    "easytier",
    "default",
    "public",
];

impl OwnerName {
    /// Parses `input`.
    ///
    /// # Errors
    ///
    /// [`DeployError`] for [`Field::NodeOwner`]: wrong length or characters, or a [`Reason::Reserved`] name.
    pub fn parse(input: &str) -> Result<Self, DeployError> {
        let text = input.trim();
        if text.chars().count() < 3 {
            return Err(err(Field::NodeOwner, Reason::TooShort));
        }
        if text.chars().count() > 32 {
            return Err(err(Field::NodeOwner, Reason::TooLong));
        }
        let ok = text
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
        if !ok {
            return Err(err(Field::NodeOwner, Reason::BadCharacter));
        }
        if RESERVED_OWNERS.contains(&text) {
            return Err(err(Field::NodeOwner, Reason::Reserved));
        }
        Ok(Self(text.to_owned()))
    }
}

text_newtype!(
    /// A lower-case hex sha256 (64 digits), without an algorithm prefix.
    Sha256Hex
);

impl Sha256Hex {
    fn parse_as(input: &str, field: Field) -> Result<Self, DeployError> {
        let text = input.trim();
        let text = text
            .strip_prefix("sha256:")
            .unwrap_or(text)
            .to_ascii_lowercase();
        if text.len() != 64 {
            return Err(err(field, Reason::BadFormat));
        }
        if !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(err(field, Reason::BadCharacter));
        }
        Ok(Self(text))
    }

    /// Parses the checksum of a release zip; an optional `sha256:` prefix is accepted.
    ///
    /// # Errors
    ///
    /// [`DeployError`] for [`Field::AssetSha256`].
    pub fn parse(input: &str) -> Result<Self, DeployError> {
        Self::parse_as(input, Field::AssetSha256)
    }

    /// Parses an image index digest (`sha256:<hex>` or bare hex).
    ///
    /// # Errors
    ///
    /// [`DeployError`] for [`Field::ImageDigest`].
    pub fn parse_digest(input: &str) -> Result<Self, DeployError> {
        Self::parse_as(input, Field::ImageDigest)
    }
}

text_newtype!(
    /// A container image tag (`[A-Za-z0-9_.-]`, 1-128 characters, not starting with `.` or `-`).
    ImageTag
);

impl ImageTag {
    /// Parses `input`.
    ///
    /// # Errors
    ///
    /// [`DeployError`] for [`Field::ImageTag`].
    pub fn parse(input: &str) -> Result<Self, DeployError> {
        let text = input.trim();
        if text.is_empty() {
            return Err(err(Field::ImageTag, Reason::Empty));
        }
        if text.len() > 128 {
            return Err(err(Field::ImageTag, Reason::TooLong));
        }
        if !text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
        {
            return Err(err(Field::ImageTag, Reason::BadCharacter));
        }
        if text.starts_with(['.', '-']) {
            return Err(err(Field::ImageTag, Reason::BadFormat));
        }
        Ok(Self(text.to_owned()))
    }
}

text_newtype!(
    /// `major.minor.patch` of an EasyTier release, digits only.
    ReleaseVersion
);

impl ReleaseVersion {
    /// Parses `2.6.4` (a leading `v` is accepted and dropped).
    ///
    /// # Errors
    ///
    /// [`DeployError`] for [`Field::Version`].
    pub fn parse(input: &str) -> Result<Self, DeployError> {
        let text = input.trim();
        let text = text.strip_prefix('v').unwrap_or(text);
        if text.is_empty() {
            return Err(err(Field::Version, Reason::Empty));
        }
        let mut parts = 0_u32;
        for part in text.split('.') {
            if part.is_empty() || part.len() > 4 || !part.bytes().all(|b| b.is_ascii_digit()) {
                return Err(err(Field::Version, Reason::BadFormat));
            }
            parts = parts.saturating_add(1);
        }
        if parts != 3 {
            return Err(err(Field::Version, Reason::BadFormat));
        }
        Ok(Self(text.to_owned()))
    }
}

text_newtype!(
    /// A name for a Docker volume or a systemd `StateDirectory=`: `[a-z0-9][a-z0-9_-]{0,62}`.
    ResourceName
);

impl ResourceName {
    /// Parses `input` as the name for `field` ([`Field::DataVolume`] or [`Field::StateDir`]).
    ///
    /// # Errors
    ///
    /// [`DeployError`] for `field`.
    pub fn parse(input: &str, field: Field) -> Result<Self, DeployError> {
        let text = input.trim();
        if text.is_empty() {
            return Err(err(field, Reason::Empty));
        }
        if text.len() > 63 {
            return Err(err(field, Reason::TooLong));
        }
        if !text
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        {
            return Err(err(field, Reason::BadCharacter));
        }
        if text.starts_with(['-', '_']) {
            return Err(err(field, Reason::BadFormat));
        }
        Ok(Self(text.to_owned()))
    }
}

text_newtype!(
    /// An absolute, normalized path made of `[A-Za-z0-9._-]` segments (no `..`, not under system directories).
    AbsPath
);

/// Top-level directories a public web root or a mounted build must never point into.
const FORBIDDEN_ROOTS: [&str; 11] = [
    "etc", "root", "proc", "sys", "dev", "boot", "bin", "sbin", "lib", "lib64", "run",
];

impl AbsPath {
    /// Parses `input` as the path for `field` ([`Field::WebDir`]).
    ///
    /// # Errors
    ///
    /// [`DeployError`] for `field`.
    pub fn parse(input: &str, field: Field) -> Result<Self, DeployError> {
        let text = input.trim();
        if text.is_empty() {
            return Err(err(field, Reason::Empty));
        }
        if text.len() > 200 {
            return Err(err(field, Reason::TooLong));
        }
        let Some(rest) = text.strip_prefix('/') else {
            return Err(err(field, Reason::BadFormat));
        };
        let mut first = true;
        for segment in rest.split('/') {
            if segment.is_empty() || segment == "." || segment == ".." {
                return Err(err(field, Reason::BadFormat));
            }
            if !segment
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
            {
                return Err(err(field, Reason::BadCharacter));
            }
            if first && FORBIDDEN_ROOTS.contains(&segment) {
                return Err(err(field, Reason::Reserved));
            }
            first = false;
        }
        Ok(Self(text.to_owned()))
    }
}

text_newtype!(
    /// An https URL of a release zip, without credentials, query or fragment.
    AssetUrl
);

impl AssetUrl {
    /// Parses `input`.
    ///
    /// # Errors
    ///
    /// [`DeployError`] for [`Field::DownloadUrl`].
    pub fn parse(input: &str) -> Result<Self, DeployError> {
        let url =
            Url::parse(input.trim()).map_err(|_| err(Field::DownloadUrl, Reason::BadFormat))?;
        if url.scheme() != "https" {
            return Err(err(Field::DownloadUrl, Reason::Insecure));
        }
        if url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(err(Field::DownloadUrl, Reason::Insecure));
        }
        Ok(Self(url.into()))
    }

    /// The file name in the URL's last path segment (`easytier-linux-x86_64-v2.6.4.zip`).
    pub fn file_name(&self) -> Option<String> {
        let url = Url::parse(&self.0).ok()?;
        url.path_segments()?.next_back().map(str::to_owned)
    }
}

/// The reverse proxy in front of easytier-web and the web build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyKind {
    /// Caddy: automatic HTTPS for the domain.
    Caddy,
    /// nginx: certificates come from the operator (certbot), paths are derived from the domain.
    Nginx,
}

/// The config server's transport. All three bind every interface; easytier-web has no bind-address flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigProtocol {
    /// UDP (easytier-web's default).
    Udp,
    /// TCP.
    Tcp,
    /// WebSocket over TCP.
    Ws,
}

impl ConfigProtocol {
    /// The value of `--config-server-protocol` and the scheme of the enrollment URL.
    pub const fn scheme(self) -> &'static str {
        match self {
            Self::Udp => "udp",
            Self::Tcp => "tcp",
            Self::Ws => "ws",
        }
    }

    /// The transport a firewall rule or port mapping names: `udp` or `tcp`.
    pub const fn transport(self) -> &'static str {
        match self {
            Self::Udp => "udp",
            Self::Tcp | Self::Ws => "tcp",
        }
    }
}

/// Where the first-run hardening stands. It is the one switch that opens the API at the proxy, turns registration
/// off and opens the config server to the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Hardening {
    /// The seeded `admin` / `user` accounts may still have their default passwords. The proxy answers 403 on
    /// `/api/`, the API is reachable over an SSH tunnel only, registration is open (to create the owner account),
    /// the config server is not opened in the firewall.
    #[default]
    Pending,
    /// Both seeded passwords were replaced and the owner account exists: `/api/` is proxied, registration is off,
    /// the config-server port is opened.
    Done,
}

/// Linux CPU architectures whose release zip layout was verified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxArch {
    /// `x86_64`.
    X86_64,
    /// `aarch64`.
    Aarch64,
}

impl LinuxArch {
    /// The architecture word in the asset name.
    pub const fn asset_word(self) -> &'static str {
        match self {
            Self::X86_64 => "x86_64",
            Self::Aarch64 => "aarch64",
        }
    }
}

/// The official container image, pinned by digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposeImage {
    /// Tag, for humans (`v2.6.4`); the digest is what is pulled.
    pub tag: ImageTag,
    /// Index digest of the tag.
    pub digest: Sha256Hex,
}

impl ComposeImage {
    /// The image pinned in [`images`].
    ///
    /// # Errors
    ///
    /// [`DeployError`] if the constants in [`images`] are malformed.
    pub fn pinned() -> Result<Self, DeployError> {
        Ok(Self {
            tag: ImageTag::parse(images::EASYTIER_TAG)?,
            digest: Sha256Hex::parse_digest(images::EASYTIER_DIGEST)?,
        })
    }

    /// `easytier/easytier:<tag>@sha256:<digest>`.
    pub fn reference(&self) -> String {
        format!(
            "{}:{}@sha256:{}",
            images::EASYTIER_IMAGE,
            self.tag,
            self.digest
        )
    }
}

/// A release zip installed under `/opt/easytier/<version>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemdRelease {
    /// Release version.
    pub version: ReleaseVersion,
    /// CPU architecture of the server.
    pub arch: LinuxArch,
    /// sha256 of the zip, checked before it is unzipped.
    pub sha256: Sha256Hex,
    /// Where the zip is downloaded from.
    pub download_url: AssetUrl,
}

impl SystemdRelease {
    /// The zip's file name: `easytier-linux-<arch>-v<version>.zip`.
    pub fn asset_name(&self) -> String {
        format!(
            "easytier-linux-{}-v{}.zip",
            self.arch.asset_word(),
            self.version
        )
    }

    /// The directory inside the zip (version is not part of it).
    pub fn asset_dir(&self) -> String {
        format!("easytier-linux-{}", self.arch.asset_word())
    }

    /// The pinned release for `arch`.
    ///
    /// # Errors
    ///
    /// [`DeployError`] if the constants in [`images`] are malformed.
    pub fn pinned(arch: LinuxArch) -> Result<Self, DeployError> {
        let version = ReleaseVersion::parse(images::EASYTIER_VERSION)?;
        let sha = match arch {
            LinuxArch::X86_64 => images::LINUX_X86_64_SHA256,
            LinuxArch::Aarch64 => images::LINUX_AARCH64_SHA256,
        };
        let url = format!(
            "{}/{}/easytier-linux-{}-{}.zip",
            images::RELEASE_BASE_URL,
            images::EASYTIER_TAG,
            arch.asset_word(),
            images::EASYTIER_TAG
        );
        Ok(Self {
            version,
            arch,
            sha256: Sha256Hex::parse(sha)?,
            download_url: AssetUrl::parse(&url)?,
        })
    }
}

/// How easytier-web runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Runtime {
    /// Docker Compose with the official image.
    Compose(ComposeImage),
    /// Release binaries under systemd.
    Systemd(SystemdRelease),
}

/// The server also joins as a node (a relay): `easytier-core` enrolled to the local easytier-web.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeService {
    /// easytier-web username the node enrolls under (the `<owner>` in the enrollment URL). The account must be
    /// registered during hardening.
    pub owner: OwnerName,
    /// Stable machine id, so the server keeps its identity when the container is recreated. Omitted: the core picks.
    pub machine_id: Option<Uuid>,
}

/// Names and locations on the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    /// Docker volume for the database (compose).
    pub data_volume: ResourceName,
    /// Name below `/var/lib` for the database (systemd `StateDirectory=`).
    pub state_dir: ResourceName,
    /// Where EasyTierUI's `nsh package --target web` output is placed; served read-only by the proxy.
    pub web_dir: AbsPath,
}

impl Paths {
    /// `easytier-web-data`, `easytier-web`, `/srv/easytierui`.
    ///
    /// # Errors
    ///
    /// [`DeployError`] never in practice; the defaults are valid.
    pub fn standard() -> Result<Self, DeployError> {
        Ok(Self {
            data_volume: ResourceName::parse("easytier-web-data", Field::DataVolume)?,
            state_dir: ResourceName::parse("easytier-web", Field::StateDir)?,
            web_dir: AbsPath::parse("/srv/easytierui", Field::WebDir)?,
        })
    }
}

/// Default REST API port.
pub const DEFAULT_API_PORT: u16 = 11211;
/// Default config-server port.
pub const DEFAULT_CONFIG_PORT: u16 = 22020;

/// Everything the renderers need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentPlan {
    /// Public domain served over HTTPS.
    pub domain: Hostname,
    /// ACME account email (Caddy); optional.
    pub acme_email: Option<Email>,
    /// Reverse proxy.
    pub proxy: ProxyKind,
    /// Container or systemd runtime.
    pub runtime: Runtime,
    /// REST API port (loopback / compose-network only).
    pub api_port: u16,
    /// Config-server port (binds all interfaces).
    pub config_port: u16,
    /// Config-server transport.
    pub config_protocol: ConfigProtocol,
    /// The server's own node, if any.
    pub node: Option<NodeService>,
    /// Volume, state directory and web build locations.
    pub paths: Paths,
    /// Hardening progress.
    pub hardening: Hardening,
}

impl DeploymentPlan {
    /// A compose plan with the pinned images and standard ports and paths.
    ///
    /// # Errors
    ///
    /// [`DeployError`] never in practice; the pinned constants are valid.
    pub fn compose(domain: Hostname, proxy: ProxyKind) -> Result<Self, DeployError> {
        Self::with_runtime(domain, proxy, Runtime::Compose(ComposeImage::pinned()?))
    }

    /// A systemd plan with the pinned release for `arch`, standard ports and paths.
    ///
    /// # Errors
    ///
    /// [`DeployError`] never in practice; the pinned constants are valid.
    pub fn systemd(
        domain: Hostname,
        proxy: ProxyKind,
        arch: LinuxArch,
    ) -> Result<Self, DeployError> {
        Self::with_runtime(
            domain,
            proxy,
            Runtime::Systemd(SystemdRelease::pinned(arch)?),
        )
    }

    fn with_runtime(
        domain: Hostname,
        proxy: ProxyKind,
        runtime: Runtime,
    ) -> Result<Self, DeployError> {
        Ok(Self {
            domain,
            acme_email: None,
            proxy,
            runtime,
            api_port: DEFAULT_API_PORT,
            config_port: DEFAULT_CONFIG_PORT,
            config_protocol: ConfigProtocol::Udp,
            node: None,
            paths: Paths::standard()?,
            hardening: Hardening::Pending,
        })
    }

    /// Checks what no single newtype can: ports, and the download URL against the selected release.
    ///
    /// # Errors
    ///
    /// [`DeployError`] for the first field at fault.
    pub fn validate(&self) -> Result<(), DeployError> {
        for (field, port) in [
            (Field::ApiPort, self.api_port),
            (Field::ConfigPort, self.config_port),
        ] {
            if port == 0 {
                return Err(err(field, Reason::OutOfRange));
            }
            if port == 80 || port == 443 {
                return Err(err(field, Reason::Conflict));
            }
        }
        if self.api_port == self.config_port {
            return Err(err(Field::ConfigPort, Reason::Conflict));
        }
        // The embedded node's RPC portal sits on this loopback port in the same network namespace.
        if self.api_port == RPC_PORTAL_PORT || self.config_port == RPC_PORTAL_PORT {
            let field = if self.api_port == RPC_PORTAL_PORT {
                Field::ApiPort
            } else {
                Field::ConfigPort
            };
            return Err(err(field, Reason::Conflict));
        }
        if let Runtime::Systemd(release) = &self.runtime
            && release.download_url.file_name().as_deref() != Some(release.asset_name().as_str())
        {
            return Err(err(Field::DownloadUrl, Reason::Mismatch));
        }
        Ok(())
    }
}

/// Loopback port of the server node's RPC portal. Always `127.0.0.1`, never published.
pub const RPC_PORTAL_PORT: u16 = 15888;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostname_rules() {
        assert_eq!(
            Hostname::parse(" Vpn.Example.COM ").unwrap().as_str(),
            "vpn.example.com"
        );
        for bad in [
            "",
            "localhost",
            "10.0.0.1",
            "a..b",
            "-a.example.com",
            "a_b.example.com",
            "ex ample.com",
            "a.com\nb.com",
        ] {
            assert!(Hostname::parse(bad).is_err(), "{bad:?}");
        }
        assert!(Hostname::parse("xn--fiq228c.example").is_ok());
    }

    #[test]
    fn email_rules() {
        assert!(Email::parse("ops@example.com").is_ok());
        for bad in [
            "ops",
            "@example.com",
            "o ps@example.com",
            "ops@localhost",
            "o\"ps@example.com",
            "ops@example.com\nimport x",
        ] {
            assert!(Email::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn owner_rules() {
        assert!(OwnerName::parse("relay-9f3a").is_ok());
        for bad in [
            "admin",
            "user",
            "ab",
            "Relay",
            "a b c",
            "x".repeat(33).as_str(),
            "a/b/c",
        ] {
            assert!(OwnerName::parse(bad).is_err(), "{bad:?}");
        }
        assert_eq!(
            OwnerName::parse("admin").unwrap_err().reason,
            Reason::Reserved
        );
    }

    #[test]
    fn path_and_digest_rules() {
        assert!(AbsPath::parse("/srv/easytierui", Field::WebDir).is_ok());
        for bad in [
            "srv/x",
            "/",
            "/srv/../etc",
            "/srv/a b",
            "/etc/x",
            "/srv//x",
            "/srv/$x",
        ] {
            assert!(AbsPath::parse(bad, Field::WebDir).is_err(), "{bad:?}");
        }
        assert!(Sha256Hex::parse_digest("sha256:abc").is_err());
        assert!(Sha256Hex::parse_digest(&format!("sha256:{}", "g".repeat(64))).is_err());
        assert!(ReleaseVersion::parse("v2.6.4").is_ok());
        assert!(ReleaseVersion::parse("2.6").is_err());
        assert!(ReleaseVersion::parse("2.6.4; rm").is_err());
    }

    #[test]
    fn url_rules() {
        assert!(AssetUrl::parse("http://github.com/x.zip").is_err());
        assert!(AssetUrl::parse("https://u:p@github.com/x.zip").is_err());
        assert!(AssetUrl::parse("https://github.com/x.zip?token=1").is_err());
        assert!(AssetUrl::parse("https://github.com/x.zip").is_ok());
    }

    #[test]
    fn plan_validation() {
        let mut plan = DeploymentPlan::compose(
            Hostname::parse("vpn.example.com").unwrap(),
            ProxyKind::Caddy,
        )
        .unwrap();
        assert!(plan.validate().is_ok());
        plan.api_port = 443;
        assert_eq!(plan.validate().unwrap_err().field, Field::ApiPort);
        plan.api_port = plan.config_port;
        assert!(plan.validate().is_err());
        let mut sd = DeploymentPlan::systemd(
            Hostname::parse("vpn.example.com").unwrap(),
            ProxyKind::Nginx,
            LinuxArch::Aarch64,
        )
        .unwrap();
        assert!(sd.validate().is_ok());
        if let Runtime::Systemd(r) = &mut sd.runtime {
            r.download_url = AssetUrl::parse("https://evil.example/payload.zip").unwrap();
        }
        assert_eq!(sd.validate().unwrap_err().reason, Reason::Mismatch);
    }
}

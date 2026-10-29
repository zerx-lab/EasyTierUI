//! Pinned third-party artifacts the templates reference. Updating EasyTier means changing this file (and the
//! fixtures), never a template.
//!
//! Digests are the multi-architecture index digests, read from the registry API on [`CHECKED_ON`]
//! (`HEAD /v2/<repo>/manifests/<tag>` with the OCI index `Accept` header, `Docker-Content-Digest`).

/// Date the digests and release checksums below were read from their registries / GitHub.
pub const CHECKED_ON: &str = "2026-10-29";

/// The official EasyTier image on Docker Hub. Its entrypoint is `tini -- easytier-core`; `easytier-web` and
/// `easytier-cli` sit beside it in `/usr/local/bin`.
pub const EASYTIER_IMAGE: &str = "easytier/easytier";
/// EasyTier release the pinned image and release checksums are for.
pub const EASYTIER_VERSION: &str = "2.6.4";
/// Image tag of [`EASYTIER_VERSION`].
pub const EASYTIER_TAG: &str = "v2.6.4";
/// Index digest (hex) of `easytier/easytier:v2.6.4`.
pub const EASYTIER_DIGEST: &str =
    "91d6732ea5ff71542fdb99333d2ee7b168e973443114db84db061e452427d91e";

/// Caddy image, tag and index digest (hex) of `caddy:2.11.7-alpine`.
pub const CADDY_IMAGE: &str = "caddy";
/// Tag of the pinned Caddy image.
pub const CADDY_TAG: &str = "2.11.7-alpine";
/// Index digest (hex) of the pinned Caddy image.
pub const CADDY_DIGEST: &str = "eb4d35b7452d1ab6171fbdbe8d9275b2301dad6dad92115d91e19fb9b24d510f";

/// nginx image, tag and index digest (hex) of `nginx:1.31.8-alpine`.
pub const NGINX_IMAGE: &str = "nginx";
/// Tag of the pinned nginx image.
pub const NGINX_TAG: &str = "1.31.8-alpine";
/// Index digest (hex) of the pinned nginx image.
pub const NGINX_DIGEST: &str = "785835ea65db4bba6f021acc0f3be91b0fef76a53fe41f8588c6698d2aed0606";

/// sha256 of `easytier-linux-x86_64-v2.6.4.zip` (GitHub release asset `digest` field).
pub const LINUX_X86_64_SHA256: &str =
    "61b659eaedba658fa66fe47d17e1426cdd77e5d02fa15fed447bb4357c09dfd6";
/// sha256 of `easytier-linux-aarch64-v2.6.4.zip`.
pub const LINUX_AARCH64_SHA256: &str =
    "f533ec25a7ea714e09f645615012200278058525795cc3bb690ff011aec1a70f";

/// Where release assets are published.
pub const RELEASE_BASE_URL: &str = "https://github.com/EasyTier/EasyTier/releases/download";

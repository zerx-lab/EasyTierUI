//! The control-plane host role on the desktops: the official `easytier-web` of the active release, run as the app's
//! child process and hardened on first run.
//!
//! - [`HostedWeb`] starts, stops and watches the process ([`HostState`]) with [`HostSettings`] persisted in
//!   `<data>/web/settings.json`. The REST API always binds `127.0.0.1`; the config server binds every interface
//!   (easytier-web has no option for it), which [`HardeningReport`] states and the UI must say.
//! - The hosted server starts with `--disable-registration` from its very first start. [`HostedWeb::harden`]
//!   replaces the seeded `admin` / `admin` and `user` / `user` passwords (kept in the
//!   [`SecretStore`](crate::core::keychain::SecretStore) as `host/admin-password`, `host/user-password`); an account
//!   someone else changed first is an error, not "replaced".
//! - The **owner account** (`owner-<10 base32>`, CSPRNG password; `host/owner-name`, `host/owner-password`) is what
//!   machines enroll under: [`HostedWeb::begin_owner_registration`] opens a loopback-only registration window and
//!   returns the captcha PNG, [`HostedWeb::complete_owner_registration`] submits the typed text and closes the window,
//!   [`HostedWeb::owner_enrollment_url`] builds the enrollment URL.
//! - [`HostedWeb::api_access`] hands the console what it needs to manage this server over loopback.
//! - A version switch applies on [`HostedWeb::restart`].
//!
//! Every [`HostedWeb`] method blocks except the plain getters: the store calls them through
//! [`work::off_thread`](crate::core::work::off_thread). Nothing outside `core` names a flag or endpoint of
//! easytier-web (AGENTS.md, "Roles and the core boundary").

mod error;
mod harden;
mod host;
mod http;
mod logs;
mod owner;
mod settings;

pub use error::{HostError, HostPort, PasswordProblem};
pub use harden::{
    ADMIN_PASSWORD_ACCOUNT, HardenRequest, HardeningReport, HostAccount, MIN_PASSWORD_CHARS,
    USER_PASSWORD_ACCOUNT,
};
pub use host::{ApiAccess, HostState, HostedWeb};
pub use logs::redact;
pub use owner::{
    ENROLLMENT_HOST_PLACEHOLDER, OWNER_NAME_ACCOUNT, OWNER_PASSWORD_ACCOUNT, OwnerError,
};
pub use settings::{
    DEFAULT_API_PORT, DEFAULT_CONFIG_PORT, HostProtocol, HostSettings, MIN_PORT, WebLayout,
};

#[cfg(test)]
mod smoke;

//! The EasyTier programs inside a release directory and asking them for their version.

use std::{
    io::{ErrorKind, Read as _},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use super::error::InstallError;
use crate::core::version::CoreVersion;

/// The node program's name without the platform's executable suffix.
pub(crate) const CORE: &str = "easytier-core";
/// The CLI's name without the suffix.
pub(crate) const CLI: &str = "easytier-cli";
/// The control-plane program's name without the suffix.
pub(crate) const WEB: &str = "easytier-web";
/// Every file of a release that must be executable. `easytier-web-embed` ships in the zip too and is run by the
/// control-plane slice, so it gets the same bit.
#[cfg(unix)]
pub(crate) const EXECUTABLES: [&str; 4] = [CORE, CLI, WEB, "easytier-web-embed"];

/// How long `--version` may take before the program counts as broken.
const VERSION_TIMEOUT: Duration = Duration::from_secs(10);
/// How often a running `--version` is checked for exit.
const VERSION_POLL: Duration = Duration::from_millis(10);

/// `name` with the platform's executable suffix (`easytier-core.exe` on Windows).
pub(crate) fn file_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

/// The three programs of a release in `dir`.
///
/// # Errors
///
/// [`InstallError::MissingProgram`] naming the first one that is not a regular file.
pub(crate) fn locate(dir: &Path) -> Result<[PathBuf; 3], InstallError> {
    let find = |name: &str| {
        let file = file_name(name);
        let path = dir.join(&file);
        if std::fs::metadata(&path).is_ok_and(|meta| meta.is_file()) {
            Ok(path)
        } else {
            Err(InstallError::MissingProgram(file))
        }
    };
    Ok([find(CORE)?, find(CLI)?, find(WEB)?])
}

/// What `program --version` reports.
///
/// The program is run directly (no shell) with no stdin and a timeout; a hung or crashing program is an error,
/// never a hang of the caller.
///
/// # Errors
///
/// [`InstallError::VersionUnreadable`] when it cannot start, times out, exits unsuccessfully or prints no version.
pub(crate) fn read_version(program: &Path) -> Result<CoreVersion, InstallError> {
    let name = program
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let unreadable = |reason: String| InstallError::VersionUnreadable {
        program: name.clone(),
        reason,
    };

    let mut command = Command::new(program);
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        /// `CREATE_NO_WINDOW`: no console flashes up when the app runs a console program.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    // A program unpacked a moment ago can still be "busy" for writing (ETXTBSY) while a concurrent fork inherits
    // its descriptor; that clears within milliseconds, so a few retries are right.
    let mut attempts = 0_u8;
    let mut child = loop {
        match command.spawn() {
            Ok(child) => break child,
            Err(error) if error.kind() == ErrorKind::ExecutableFileBusy && attempts < 5 => {
                attempts = attempts.saturating_add(1);
                thread::sleep(Duration::from_millis(20));
            }
            Err(error) => return Err(unreadable(error.to_string())),
        }
    };
    let Some(stdout) = child.stdout.take() else {
        return Err(unreadable("no output pipe".to_owned()));
    };
    // The reader runs apart so a program that never exits cannot block on a full pipe or hide its timeout.
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        // At most 64 KiB: a version line is short, anything more is not a version line.
        match stdout.take(65_536).read_to_end(&mut bytes) {
            Ok(_) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(_) => String::new(),
        }
    });

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() >= VERSION_TIMEOUT => {
                if let Err(error) = child.kill() {
                    log::warn!("could not stop {name} after its --version timed out: {error}");
                }
                if let Err(error) = child.wait() {
                    log::warn!("could not reap {name}: {error}");
                }
                return Err(unreadable("it did not answer in time".to_owned()));
            }
            Ok(None) => thread::sleep(VERSION_POLL),
            Err(error) => return Err(unreadable(error.to_string())),
        }
    };
    let output = reader
        .join()
        .map_err(|_| unreadable("reading its output failed".to_owned()))?;
    if !status.success() {
        return Err(unreadable(format!("it exited with {status}")));
    }
    CoreVersion::from_version_output(&output).map_err(|error| unreadable(error.to_string()))
}

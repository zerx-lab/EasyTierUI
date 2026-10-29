//! The app's data directory and owner-only files in it: rendered configs, logs, installed EasyTier releases,
//! easytier-web's database. Native targets only; the web build keeps nothing on disk.

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::OnceLock,
};

/// The directory name under the OS's per-user data directory.
const APP_DIR: &str = "EasyTierUI";

/// A data directory chosen before first use (tests, a portable install); see [`set_data_dir`].
static OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

/// Why the data directory is not usable.
#[derive(Debug, thiserror::Error)]
pub enum PathError {
    /// The OS reports no per-user data directory (Android without an override, the web).
    #[error("this platform has no data directory for the app")]
    NoDataDir,
    /// It could not be created or secured.
    #[error("the data directory {path}: {source}")]
    Io {
        /// The directory.
        path: PathBuf,
        /// The OS error.
        source: io::Error,
    },
}

/// Uses `dir` as the data directory from now on; only the first call counts (before any file is written).
pub fn set_data_dir(dir: PathBuf) {
    if OVERRIDE.set(dir).is_err() {
        log::warn!("the data directory was already chosen; keeping it");
    }
}

/// The app's data directory, created owner-only (`0700` on Unix; on Windows the per-user `%APPDATA%` ACL).
///
/// # Errors
///
/// [`PathError::NoDataDir`] where the OS has none, [`PathError::Io`] when it cannot be created.
pub fn data_dir() -> Result<PathBuf, PathError> {
    let dir = match OVERRIDE.get() {
        Some(dir) => dir.clone(),
        None => os_data_dir().ok_or(PathError::NoDataDir)?.join(APP_DIR),
    };
    ensure_private_dir(&dir).map_err(|source| PathError::Io {
        path: dir.clone(),
        source,
    })?;
    Ok(dir)
}

/// `name` under the data directory, created owner-only.
///
/// # Errors
///
/// As [`data_dir`].
pub fn data_subdir(name: &str) -> Result<PathBuf, PathError> {
    let dir = data_dir()?.join(name);
    ensure_private_dir(&dir).map_err(|source| PathError::Io {
        path: dir.clone(),
        source,
    })?;
    Ok(dir)
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn os_data_dir() -> Option<PathBuf> {
    dirs::data_dir()
}

#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn os_data_dir() -> Option<PathBuf> {
    None
}

/// Creates `dir` and its parents; on Unix, narrows `dir` to `0700`.
///
/// # Errors
///
/// The OS error.
pub fn ensure_private_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Writes `bytes` to `path` atomically (a temporary file in the same directory, then a rename) with owner-only
/// permissions (`0600` on Unix). Readers never see a half-written config.
///
/// # Errors
///
/// The OS error; the temporary file is removed on failure.
pub fn write_private_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write as _;

    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "a file path needs a parent"))?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "a file path needs a name"))?;
    let mut temp_name = name.to_os_string();
    temp_name.push(".tmp");
    let temp = parent.join(temp_name);

    let written = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut file = options.open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&temp, fs::Permissions::from_mode(0o600))?;
        }
        fs::rename(&temp, path)
    })();
    if written.is_err()
        && temp.exists()
        && let Err(error) = fs::remove_file(&temp)
    {
        log::warn!("could not remove {}: {error}", temp.display());
    }
    written
}

/// Removes `path` if it exists.
///
/// # Errors
///
/// The OS error, other than "not found".
pub fn remove_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_files_are_written_whole_and_owner_only() {
        let dir = std::env::temp_dir().join(format!("etui-paths-{}", std::process::id()));
        ensure_private_dir(&dir).unwrap();
        let file = dir.join("node.toml");
        write_private_file(&file, b"a = 1\n").unwrap();
        write_private_file(&file, b"a = 2\n").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "a = 2\n");
        assert!(!dir.join("node.toml.tmp").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
            let dir_mode = fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
            assert_eq!(dir_mode, 0o700);
        }
        remove_if_exists(&file).unwrap();
        remove_if_exists(&file).unwrap();
        fs::remove_dir(&dir).unwrap();
    }
}

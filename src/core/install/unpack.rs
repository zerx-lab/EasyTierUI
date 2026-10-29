//! Unpacking an EasyTier release zip into a directory the caller owns.
//!
//! The zip comes from the network and was only checked against a digest the same network path delivered, so every
//! entry is treated as hostile: paths are validated before anything is created, only regular files and directories
//! are written, sizes are capped, and nothing is ever created outside the destination.

use std::{
    fs,
    io::{Cursor, Read as _, Write as _},
    path::Path,
};

use zip::ZipArchive;

use super::error::InstallError;
use crate::core::paths::ensure_private_dir;

/// The most entries a release zip may hold (the real ones hold 5 to 10).
const MAX_ENTRIES: usize = 256;
/// The most bytes all entries together may unpack to (the real ones are about 75 MB).
const MAX_UNPACKED_BYTES: u64 = 512 * 1024 * 1024;
/// Copy buffer size.
const CHUNK: usize = 64 * 1024;

/// `st_mode` file-type mask and the two types allowed in a release.
const S_IFMT: u32 = 0o170_000;
const S_IFREG: u32 = 0o100_000;
const S_IFDIR: u32 = 0o040_000;

/// One validated entry.
struct Planned {
    /// Index in the archive.
    index: usize,
    /// The path's segments, none of them `..`, empty, `.` or absolute.
    segments: Vec<String>,
    /// A directory entry (created on demand, nothing to write).
    is_dir: bool,
}

/// Splits an entry name into safe path segments.
///
/// # Errors
///
/// [`InstallError::BadEntry`] for absolute names, `..`, drive letters and NUL.
fn split_name(name: &str) -> Result<Vec<String>, InstallError> {
    let bad = || InstallError::BadEntry(name.chars().take(80).collect());
    // Zips made on Windows use backslashes; treat them as separators on every platform so one cannot hide a `..`.
    let normalized = name.replace('\\', "/");
    if normalized.starts_with('/') || normalized.contains('\0') {
        return Err(bad());
    }
    let mut segments = Vec::new();
    for segment in normalized.split('/') {
        match segment {
            "" | "." => {}
            ".." => return Err(bad()),
            other if other.contains(':') => return Err(bad()),
            other => segments.push(other.to_owned()),
        }
    }
    Ok(segments)
}

/// Unpacks `bytes` (a zip) into `dest`, stripping the zip's single top-level directory.
///
/// `dest` is created owner-only if it does not exist and must not hold the release's files yet. On error the
/// caller deletes `dest`: a partial unpack is never used.
///
/// # Errors
///
/// [`InstallError::BadArchive`] when the zip is damaged, too big or has duplicate entries,
/// [`InstallError::BadEntry`] for an entry that would leave `dest` or is not a regular file, [`InstallError::Io`]
/// for write failures.
pub(crate) fn unpack_zip(bytes: &[u8], dest: &Path) -> Result<(), InstallError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| InstallError::BadArchive(error.to_string()))?;
    if archive.len() > MAX_ENTRIES {
        return Err(InstallError::BadArchive(format!(
            "{} entries",
            archive.len()
        )));
    }

    // Pass 1: validate every entry before writing anything.
    let mut planned = Vec::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|error| InstallError::BadArchive(error.to_string()))?;
        let segments = split_name(entry.name())?;
        if let Some(mode) = entry.unix_mode() {
            let kind = mode & S_IFMT;
            if kind != 0 && kind != S_IFREG && kind != S_IFDIR {
                return Err(InstallError::BadEntry(
                    entry.name().chars().take(80).collect(),
                ));
            }
        }
        if entry.is_symlink() {
            return Err(InstallError::BadEntry(
                entry.name().chars().take(80).collect(),
            ));
        }
        if segments.is_empty() {
            continue;
        }
        planned.push(Planned {
            index,
            segments,
            is_dir: entry.is_dir(),
        });
    }

    // One shared first segment and no file directly at the top: that segment is the release's wrapper directory.
    let strip = planned.first().is_some_and(|first| {
        planned.iter().all(|entry| {
            entry.segments.first() == first.segments.first()
                && (entry.is_dir || entry.segments.len() > 1)
        })
    });
    let strip = usize::from(strip);

    // Pass 2: write.
    ensure_private_dir(dest)
        .map_err(|error| InstallError::io("creating the unpack directory", &error))?;
    let mut total: u64 = 0;
    let mut buffer = vec![0_u8; CHUNK];
    for entry in &planned {
        let Some(relative) = entry.segments.get(strip..) else {
            continue;
        };
        if relative.is_empty() {
            continue;
        }
        let mut target = dest.to_path_buf();
        target.extend(relative);
        if entry.is_dir {
            fs::create_dir_all(&target)
                .map_err(|error| InstallError::io("creating a directory", &error))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| InstallError::io("creating a directory", &error))?;
        }
        let mut source = archive
            .by_index(entry.index)
            .map_err(|error| InstallError::BadArchive(error.to_string()))?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    InstallError::BadArchive(format!("{} appears twice", relative.join("/")))
                } else {
                    InstallError::io("creating a file", &error)
                }
            })?;
        loop {
            let read = source
                .read(&mut buffer)
                .map_err(|error| InstallError::BadArchive(error.to_string()))?;
            if read == 0 {
                break;
            }
            let Some(chunk) = buffer.get(..read) else {
                break;
            };
            let len = u64::try_from(read).unwrap_or(u64::MAX);
            total = total.saturating_add(len);
            if total > MAX_UNPACKED_BYTES {
                return Err(InstallError::BadArchive(
                    "it unpacks to more than 512 MiB".to_owned(),
                ));
            }
            file.write_all(chunk)
                .map_err(|error| InstallError::io("writing a file", &error))?;
        }
        file.flush()
            .map_err(|error| InstallError::io("writing a file", &error))?;
        drop(file);
        set_mode(&target, relative)?;
    }
    Ok(())
}

/// `0755` for the release's programs (they sit at the release's top), `0644` for the rest, on Unix.
#[cfg(unix)]
fn set_mode(target: &Path, relative: &[String]) -> Result<(), InstallError> {
    use std::os::unix::fs::PermissionsExt as _;

    use super::programs::{EXECUTABLES, file_name};

    let is_program = relative.len() == 1
        && relative.first().is_some_and(|name| {
            EXECUTABLES
                .iter()
                .any(|program| *name == file_name(program))
        });
    let mode = if is_program { 0o755 } else { 0o644 };
    fs::set_permissions(target, fs::Permissions::from_mode(mode))
        .map_err(|error| InstallError::io("setting permissions", &error))
}

/// Windows has no execute bit.
#[cfg(not(unix))]
fn set_mode(_target: &Path, _relative: &[String]) -> Result<(), InstallError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use zip::{ZipWriter, write::SimpleFileOptions};

    use super::*;

    /// A zip with the given `(name, content)` files.
    fn make_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, content) in files {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(content).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("etui-unpack-{name}-{}", std::process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir).unwrap();
        }
        dir
    }

    #[test]
    fn strips_the_top_level_directory_and_marks_programs_executable() {
        let zip = make_zip(&[
            ("easytier-linux-x86_64/easytier-core", b"core"),
            ("easytier-linux-x86_64/easytier-cli", b"cli"),
            ("easytier-linux-x86_64/readme.txt", b"hi"),
            ("easytier-linux-x86_64/sub/data.bin", b"data"),
        ]);
        let dest = temp("strip");
        unpack_zip(&zip, &dest).unwrap();
        assert_eq!(fs::read(dest.join("easytier-core")).unwrap(), b"core");
        assert_eq!(fs::read(dest.join("sub/data.bin")).unwrap(), b"data");
        assert!(!dest.join("easytier-linux-x86_64").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode =
                |name: &str| fs::metadata(dest.join(name)).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode("easytier-core"), 0o755);
            assert_eq!(mode("easytier-cli"), 0o755);
            assert_eq!(mode("readme.txt"), 0o644);
        }
        fs::remove_dir_all(&dest).unwrap();
    }

    #[test]
    fn keeps_a_flat_zip_as_it_is() {
        let zip = make_zip(&[("easytier-core", b"core"), ("easytier-cli", b"cli")]);
        let dest = temp("flat");
        unpack_zip(&zip, &dest).unwrap();
        assert!(dest.join("easytier-core").is_file());
        fs::remove_dir_all(&dest).unwrap();
    }

    #[test]
    fn rejects_traversal_and_absolute_paths_before_writing() {
        for name in [
            "top/../../escape",
            "../escape",
            "/etc/escape",
            "top\\..\\..\\escape",
            "C:/escape",
        ] {
            let zip = make_zip(&[("top/easytier-core", b"core"), (name, b"x")]);
            let dest = temp("evil");
            let error = unpack_zip(&zip, &dest).unwrap_err();
            assert!(
                matches!(error, InstallError::BadEntry(_)),
                "{name}: {error:?}"
            );
            // Validation runs first: not even the harmless entry was written.
            assert!(!dest.exists(), "{name} wrote something");
        }
    }

    #[test]
    fn rejects_symlinks() {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .add_symlink("top/link", "/etc/passwd", SimpleFileOptions::default())
            .unwrap();
        let zip = writer.finish().unwrap().into_inner();
        let dest = temp("link");
        assert!(matches!(
            unpack_zip(&zip, &dest),
            Err(InstallError::BadEntry(_))
        ));
        assert!(!dest.exists());
    }

    #[test]
    fn rejects_garbage() {
        assert!(matches!(
            unpack_zip(b"not a zip", &temp("garbage")),
            Err(InstallError::BadArchive(_))
        ));
    }
}

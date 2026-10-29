//! Starting the app when the user signs in: a per-user login item, never a system service, so no administrator is
//! asked. macOS: a LaunchAgent property list in `~/Library/LaunchAgents` (launchd starts it at the next login);
//! Windows: a value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`; Linux: an XDG autostart desktop entry
//! in `$XDG_CONFIG_HOME/autostart` (default `~/.config/autostart`). The entry runs the app's executable as it is now;
//! enabling it again after the app moved points it at the new place.

#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::path::Path;
use std::path::PathBuf;

/// The app as its login item names and starts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginEntry {
    /// Reverse-DNS id (`com.example.easytierui`): the LaunchAgent's label and the desktop entry's file name.
    pub id: String,
    /// The app's name: the Run value's name and the desktop entry's `Name`.
    pub name: String,
    /// The executable to start.
    pub program: PathBuf,
}

/// Why the login item could not be read or changed.
#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    /// The OS reports no home or configuration directory for this user.
    #[error("this user has no home directory to keep a login item in")]
    NoHome,
    /// The executable's path cannot be written into the entry (not Unicode, or a line break in it).
    #[error("the app's path cannot be written into a login item: {}", .0.display())]
    UnsupportedPath(PathBuf),
    /// Reading, writing or removing the entry failed.
    #[error("{action} {}: {source}", path.display())]
    Io {
        /// What was being done.
        action: &'static str,
        /// The entry's file.
        path: PathBuf,
        /// The OS error.
        #[source]
        source: std::io::Error,
    },
    /// The registry refused (Windows).
    #[error("the registry: {0}")]
    Registry(String),
}

/// Whether the login item exists.
///
/// # Errors
///
/// [`LoginError`] when the place it lives in cannot be read.
pub fn is_enabled(entry: &LoginEntry) -> Result<bool, LoginError> {
    imp::is_enabled(entry)
}

/// Adds (`on`) or removes the login item; adding replaces an existing one.
///
/// # Errors
///
/// [`LoginError`] when it cannot be written or removed.
pub fn set_enabled(entry: &LoginEntry, on: bool) -> Result<(), LoginError> {
    if on {
        imp::enable(entry)
    } else {
        imp::disable(entry)
    }
}

/// The executable's path as text with no line breaks.
fn program_text(entry: &LoginEntry) -> Result<&str, LoginError> {
    entry
        .program
        .to_str()
        .filter(|text| !text.contains(['\n', '\r']))
        .ok_or_else(|| LoginError::UnsupportedPath(entry.program.clone()))
}

/// macOS: the LaunchAgent's property list.
pub fn launch_agent_plist(entry: &LoginEntry) -> Result<String, LoginError> {
    let program = xml_escape(program_text(entry)?);
    let label = xml_escape(&launch_agent_label(entry));
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>{label}</string>
	<key>ProgramArguments</key>
	<array>
		<string>{program}</string>
	</array>
	<key>RunAtLoad</key>
	<true/>
	<key>ProcessType</key>
	<string>Interactive</string>
	<key>LimitLoadToSessionType</key>
	<string>Aqua</string>
</dict>
</plist>
"#
    ))
}

fn launch_agent_label(entry: &LoginEntry) -> String {
    format!("{}.login", entry.id)
}

/// Linux: the XDG autostart desktop entry.
pub fn desktop_entry(entry: &LoginEntry) -> Result<String, LoginError> {
    let exec = exec_quote(program_text(entry)?);
    let name: String = entry.name.chars().filter(|c| !c.is_control()).collect();
    Ok(format!(
        "[Desktop Entry]\nType=Application\nName={name}\nExec={exec}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"
    ))
}

/// Windows: the Run value's data (the path quoted, as the shell splits it at spaces otherwise).
pub fn run_value(entry: &LoginEntry) -> Result<String, LoginError> {
    let program = program_text(entry)?;
    if program.contains('"') {
        return Err(LoginError::UnsupportedPath(entry.program.clone()));
    }
    Ok(format!("\"{program}\""))
}

fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

/// One quoted argument of a desktop entry's `Exec` key. The key's string escapes apply first, so a literal backslash
/// is four of them; `"`, `` ` `` and `$` are backslash-escaped inside the quotes, and `%` (a field code) is doubled.
fn exec_quote(argument: &str) -> String {
    let mut out = String::with_capacity(argument.len().saturating_add(2));
    out.push('"');
    for c in argument.chars() {
        match c {
            '\\' => out.push_str("\\\\\\\\"),
            '"' | '`' | '$' => {
                out.push_str("\\\\");
                out.push(c);
            }
            '%' => out.push_str("%%"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Writes `contents` to `path` through a temporary file beside it, so a login never reads half an entry.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn write_entry(path: &Path, contents: &str) -> Result<(), LoginError> {
    let io = |action| {
        move |source| LoginError::Io {
            action,
            path: path.to_owned(),
            source,
        }
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(io("create the folder of"))?;
    }
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, contents).map_err(io("write"))?;
    std::fs::rename(&temporary, path).map_err(io("write"))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn remove_entry(path: &Path) -> Result<(), LoginError> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(LoginError::Io {
            action: "remove",
            path: path.to_owned(),
            source: error,
        }),
        _ => Ok(()),
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn entry_exists(path: &Path) -> Result<bool, LoginError> {
    path.try_exists().map_err(|source| LoginError::Io {
        action: "read",
        path: path.to_owned(),
        source,
    })
}

#[cfg(target_os = "macos")]
mod imp {
    use super::*;

    fn path(entry: &LoginEntry) -> Result<PathBuf, LoginError> {
        let home = std::env::var_os("HOME")
            .filter(|home| !home.is_empty())
            .ok_or(LoginError::NoHome)?;
        Ok(Path::new(&home)
            .join("Library/LaunchAgents")
            .join(format!("{}.plist", launch_agent_label(entry))))
    }

    pub(super) fn is_enabled(entry: &LoginEntry) -> Result<bool, LoginError> {
        entry_exists(&path(entry)?)
    }

    pub(super) fn enable(entry: &LoginEntry) -> Result<(), LoginError> {
        write_entry(&path(entry)?, &launch_agent_plist(entry)?)
    }

    pub(super) fn disable(entry: &LoginEntry) -> Result<(), LoginError> {
        remove_entry(&path(entry)?)
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use super::*;

    fn path(entry: &LoginEntry) -> Result<PathBuf, LoginError> {
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|dir| Path::new(dir).is_absolute())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .filter(|home| !home.is_empty())
                    .map(|home| Path::new(&home).join(".config"))
            })
            .ok_or(LoginError::NoHome)?;
        Ok(config
            .join("autostart")
            .join(format!("{}.desktop", entry.id)))
    }

    pub(super) fn is_enabled(entry: &LoginEntry) -> Result<bool, LoginError> {
        entry_exists(&path(entry)?)
    }

    pub(super) fn enable(entry: &LoginEntry) -> Result<(), LoginError> {
        write_entry(&path(entry)?, &desktop_entry(entry)?)
    }

    pub(super) fn disable(entry: &LoginEntry) -> Result<(), LoginError> {
        remove_entry(&path(entry)?)
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use windows_registry::CURRENT_USER;

    use super::*;

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    /// `ERROR_FILE_NOT_FOUND` as an HRESULT: the value or key is absent.
    const NOT_FOUND: i32 = 0x8007_0002_u32.cast_signed();

    fn registry(error: &windows::core::Error) -> LoginError {
        LoginError::Registry(error.message())
    }

    pub(super) fn is_enabled(entry: &LoginEntry) -> Result<bool, LoginError> {
        let key = match CURRENT_USER.open(RUN) {
            Ok(key) => key,
            Err(error) if error.code().0 == NOT_FOUND => return Ok(false),
            Err(error) => return Err(registry(&error)),
        };
        match key.get_string(&entry.name) {
            Ok(_) => Ok(true),
            Err(error) if error.code().0 == NOT_FOUND => Ok(false),
            Err(error) => Err(registry(&error)),
        }
    }

    pub(super) fn enable(entry: &LoginEntry) -> Result<(), LoginError> {
        let value = run_value(entry)?;
        CURRENT_USER
            .create(RUN)
            .and_then(|key| key.set_string(&entry.name, &value))
            .map_err(|error| registry(&error))
    }

    pub(super) fn disable(entry: &LoginEntry) -> Result<(), LoginError> {
        let key = match CURRENT_USER.open(RUN) {
            Ok(key) => key,
            Err(error) if error.code().0 == NOT_FOUND => return Ok(()),
            Err(error) => return Err(registry(&error)),
        };
        match key.remove_value(&entry.name) {
            Err(error) if error.code().0 != NOT_FOUND => Err(registry(&error)),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(program: &str) -> LoginEntry {
        LoginEntry {
            id: "com.example.easytierui".into(),
            name: "EasyTierUI".into(),
            program: PathBuf::from(program),
        }
    }

    #[test]
    fn the_launch_agent_escapes_the_path() {
        let plist =
            launch_agent_plist(&entry("/Apps/A&B <x>.app/Contents/MacOS/EasyTierUI")).unwrap();
        assert!(plist.contains("<string>com.example.easytierui.login</string>"));
        assert!(
            plist
                .contains("<string>/Apps/A&amp;B &lt;x&gt;.app/Contents/MacOS/EasyTierUI</string>")
        );
        assert!(plist.contains("<key>RunAtLoad</key>\n\t<true/>"));
    }

    #[test]
    fn the_desktop_entry_quotes_exec_per_the_spec() {
        let text = desktop_entry(&entry("/opt/my app/$HOME\\100%/easytierui")).unwrap();
        assert!(
            text.contains("Exec=\"/opt/my app/\\\\$HOME\\\\\\\\100%%/easytierui\"\n"),
            "{text}"
        );
        assert!(text.starts_with("[Desktop Entry]\nType=Application\nName=EasyTierUI\n"));
    }

    #[test]
    fn the_run_value_is_quoted() {
        assert_eq!(
            run_value(&entry(r"C:\Program Files\EasyTierUI\EasyTierUI.exe")).unwrap(),
            r#""C:\Program Files\EasyTierUI\EasyTierUI.exe""#
        );
    }

    #[test]
    fn a_path_that_would_break_the_entry_is_refused() {
        for program in ["/opt/a\nExec=/bin/evil", "C:\\a\"b.exe"] {
            let entry = entry(program);
            let refused = if program.contains('"') {
                run_value(&entry)
            } else {
                desktop_entry(&entry)
            };
            assert!(
                matches!(refused, Err(LoginError::UnsupportedPath(_))),
                "{program:?}"
            );
        }
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn writing_and_removing_an_entry_round_trips() {
        let dir = std::env::temp_dir().join(format!("desktop-shell-login-{}", std::process::id()));
        let path = dir.join("autostart/x.desktop");
        assert!(!entry_exists(&path).unwrap());
        write_entry(&path, "[Desktop Entry]\n").unwrap();
        assert!(entry_exists(&path).unwrap());
        assert!(!path.with_extension("tmp").exists());
        remove_entry(&path).unwrap();
        remove_entry(&path).unwrap();
        assert!(!entry_exists(&path).unwrap());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

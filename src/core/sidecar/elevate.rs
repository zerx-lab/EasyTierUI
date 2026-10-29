//! Running one program with administrator rights through the OS's own mechanism, after the user agreed: the only
//! place the sidecar asks for elevation. The program is either the administrator-only copy of `easytier-cli`
//! (`service start / stop`) or the app's own executable with a [`super::privileged`] request (copy the service's
//! files, install, uninstall).
//!
//! No shell string is ever built from a value: each platform's launcher receives the program and its arguments as an
//! argument vector, and the small fixed scripts of macOS and Windows read them as *data* (`argv`, environment
//! variables), never as script text.
//!
//! | OS | Launcher | A cancelled prompt |
//! |---|---|---|
//! | Linux | `pkexec <program> <args…>` (polkit's prompt) | exit status 126 (dismissed) or 127 (not authorized) |
//! | macOS | `osascript` running a fixed AppleScript whose `argv` carries the program and arguments; `quoted form of` makes the one `do shell script … with administrator privileges` line | `(-128)` in the error text |
//! | Windows | Windows PowerShell running a fixed `Start-Process -Verb RunAs -Wait` script; the program and the pre-quoted argument string travel in environment variables | exit status 1223 |
//!
//! Why not a crate: `runas` 1.2 needs `sudo` and a terminal on Linux, calls the deprecated
//! `AuthorizationExecuteWithPrivileges` on macOS and mangles Windows paths with backslashes; `elevated-command` 1.1
//! joins arguments without quoting, `unwrap`s, and does not wait for the elevated process on Windows. Each also needs
//! `unsafe` in its own code; this module needs none.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    time::Duration,
};

use super::run;
use crate::core::node::NodeError;

/// How long the user may sit at the prompt (plus the command's own run time).
const ELEVATION_TIMEOUT: Duration = Duration::from_secs(180);

/// The longest message kept from an elevated command's output.
const MAX_MESSAGE_CHARS: usize = 600;

/// Windows' `ERROR_CANCELLED`: what the PowerShell script exits with when the UAC prompt was declined.
const WINDOWS_CANCELLED: i32 = 1223;

/// The default text of the macOS password prompt.
pub const DEFAULT_PROMPT: &str = "EasyTierUI wants to manage the EasyTier network service.";

/// What an elevated command printed, when the platform lets us see it (not on Windows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElevatedOutput {
    /// Standard output and error, merged, trimmed.
    pub text: String,
}

/// A way to run a program with administrator rights. The real one asks the OS; tests substitute a recorder so no
/// prompt (and no system change) ever happens.
pub trait Elevate: Send + Sync {
    /// Runs `program args…` as an administrator.
    ///
    /// # Errors
    ///
    /// [`NodeError::ElevationDenied`] when the user cancelled or the OS refused the prompt, [`NodeError::Service`]
    /// with the (redacted) output when the command failed or no prompt could be shown.
    fn run(&self, program: &Path, args: &[OsString]) -> Result<ElevatedOutput, NodeError>;
}

/// The OS's own prompt.
#[derive(Debug, Clone)]
pub struct SystemElevator {
    prompt: String,
}

impl SystemElevator {
    /// An elevator whose macOS prompt says `prompt` (already localized by the caller).
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
        }
    }
}

impl Default for SystemElevator {
    fn default() -> Self {
        Self::new(DEFAULT_PROMPT)
    }
}

/// The program and arguments that put the OS's prompt in front of `program args…`, plus extra environment variables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launcher {
    /// The launcher binary (an absolute path).
    pub program: PathBuf,
    /// Its arguments.
    pub args: Vec<OsString>,
    /// Environment variables the launcher's fixed script reads.
    pub env: Vec<(OsString, OsString)>,
}

/// Linux: `pkexec program args…`.
pub fn pkexec_launcher(pkexec: &Path, program: &Path, args: &[OsString]) -> Launcher {
    let mut all = vec![program.as_os_str().to_owned()];
    all.extend(args.iter().cloned());
    Launcher {
        program: pkexec.to_path_buf(),
        args: all,
        env: Vec::new(),
    }
}

/// The fixed AppleScript: `argv` is `[prompt, program, args…]`; every element is shell-quoted by `quoted form of`.
const APPLESCRIPT: [&str; 8] = [
    "on run argv",
    "set promptText to item 1 of argv",
    "set commandLine to \"\"",
    "repeat with anArg in (items 2 thru -1 of argv)",
    "set commandLine to commandLine & \" \" & quoted form of (anArg as text)",
    "end repeat",
    "do shell script commandLine with prompt promptText with administrator privileges",
    "end run",
];

/// macOS: `osascript -e <fixed script lines> -- <prompt> <program> <args…>`.
pub fn osascript_launcher(
    osascript: &Path,
    prompt: &str,
    program: &Path,
    args: &[OsString],
) -> Launcher {
    let mut all = Vec::new();
    for line in APPLESCRIPT {
        all.push(OsString::from("-e"));
        all.push(OsString::from(line));
    }
    all.push(OsString::from("--"));
    all.push(OsString::from(prompt));
    all.push(program.as_os_str().to_owned());
    all.extend(args.iter().cloned());
    Launcher {
        program: osascript.to_path_buf(),
        args: all,
        env: Vec::new(),
    }
}

/// The environment variable that carries the elevated program's path to the PowerShell script.
pub const WINDOWS_PROGRAM_ENV: &str = "ETUI_ELEVATE_PROGRAM";

/// The environment variable that carries the elevated program's command-line string to the PowerShell script.
pub const WINDOWS_ARGS_ENV: &str = "ETUI_ELEVATE_ARGS";

/// The fixed PowerShell script; it reads its two inputs from the environment and interpolates nothing.
const POWERSHELL_SCRIPT: &str = "$ErrorActionPreference = 'Stop'; \
try { \
$p = Start-Process -FilePath $env:ETUI_ELEVATE_PROGRAM -ArgumentList $env:ETUI_ELEVATE_ARGS -Verb RunAs -WindowStyle Hidden -Wait -PassThru; \
exit $p.ExitCode \
} catch { \
if ($_.Exception.NativeErrorCode -eq 1223) { exit 1223 } \
[Console]::Error.WriteLine($_.Exception.Message); exit 1 \
}";

/// Windows: Windows PowerShell running the fixed script, with the program and its quoted command line in the
/// environment.
pub fn powershell_launcher(powershell: &Path, program: &Path, args: &[OsString]) -> Launcher {
    Launcher {
        program: powershell.to_path_buf(),
        args: [
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            POWERSHELL_SCRIPT,
        ]
        .into_iter()
        .map(OsString::from)
        .collect(),
        env: vec![
            (
                OsString::from(WINDOWS_PROGRAM_ENV),
                program.as_os_str().to_owned(),
            ),
            (
                OsString::from(WINDOWS_ARGS_ENV),
                OsString::from(windows_command_line(args)),
            ),
        ],
    }
}

/// Quotes one argument for a Windows command line the way `CommandLineToArgvW` / the MSVC runtime read it back.
pub fn windows_quote(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '\n', '\u{b}', '"']) {
        return arg.to_owned();
    }
    let mut out = String::with_capacity(arg.len().saturating_add(2));
    out.push('"');
    let mut backslashes = 0_usize;
    for c in arg.chars() {
        match c {
            '\\' => backslashes = backslashes.saturating_add(1),
            '"' => {
                // Backslashes before a quote are doubled, and the quote itself escaped.
                out.extend(std::iter::repeat_n(
                    '\\',
                    backslashes.saturating_mul(2).saturating_add(1),
                ));
                out.push('"');
                backslashes = 0;
            }
            other => {
                out.extend(std::iter::repeat_n('\\', backslashes));
                backslashes = 0;
                out.push(other);
            }
        }
    }
    // Backslashes before the closing quote are doubled so they do not escape it.
    out.extend(std::iter::repeat_n('\\', backslashes.saturating_mul(2)));
    out.push('"');
    out
}

/// The arguments as one Windows command-line string.
pub fn windows_command_line(args: &[OsString]) -> String {
    args.iter()
        .map(|arg| windows_quote(&arg.to_string_lossy()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// How a finished launcher's result is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOs {
    /// Linux with polkit.
    Linux,
    /// macOS with `osascript`.
    MacOs,
    /// Windows with UAC through PowerShell.
    Windows,
}

/// Whether a failed launcher run means the user declined.
pub fn declined(os: HostOs, code: Option<i32>, output: &str) -> bool {
    match os {
        HostOs::Linux => matches!(code, Some(126 | 127)),
        HostOs::MacOs => {
            output.contains("(-128)") || output.to_lowercase().contains("user canceled")
        }
        HostOs::Windows => code == Some(WINDOWS_CANCELLED),
    }
}

fn current_os() -> Option<HostOs> {
    if cfg!(target_os = "linux") {
        Some(HostOs::Linux)
    } else if cfg!(target_os = "macos") {
        Some(HostOs::MacOs)
    } else if cfg!(target_os = "windows") {
        Some(HostOs::Windows)
    } else {
        None
    }
}

fn first_existing(candidates: &[&str]) -> Option<PathBuf> {
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
}

fn windows_powershell() -> Option<PathBuf> {
    let root = std::env::var_os("SystemRoot").or_else(|| std::env::var_os("windir"))?;
    let path = PathBuf::from(root)
        .join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe");
    path.is_file().then_some(path)
}

fn clip(text: &str) -> String {
    let text = text.trim();
    let clipped: String = text.chars().take(MAX_MESSAGE_CHARS).collect();
    if text.chars().count() > MAX_MESSAGE_CHARS {
        format!("{clipped}…")
    } else {
        clipped
    }
}

impl Elevate for SystemElevator {
    fn run(&self, program: &Path, args: &[OsString]) -> Result<ElevatedOutput, NodeError> {
        let (os, launcher) = match current_os() {
            Some(os @ HostOs::Linux) => (
                os,
                first_existing(&["/usr/bin/pkexec", "/bin/pkexec"])
                    .map(|pkexec| pkexec_launcher(&pkexec, program, args)),
            ),
            Some(os @ HostOs::MacOs) => (
                os,
                first_existing(&["/usr/bin/osascript"])
                    .map(|osascript| osascript_launcher(&osascript, &self.prompt, program, args)),
            ),
            Some(os @ HostOs::Windows) => (
                os,
                windows_powershell().map(|ps| powershell_launcher(&ps, program, args)),
            ),
            None => {
                return Err(NodeError::Unsupported(
                    "administrator approval is not available here",
                ));
            }
        };
        let launcher = launcher.ok_or_else(|| {
            NodeError::Service(
                "the system's administrator prompt (polkit, osascript or PowerShell) was not found"
                    .to_owned(),
            )
        })?;
        let mut command = run::command(&launcher.program, &launcher.args);
        for (name, value) in &launcher.env {
            command.env(name, value);
        }
        let captured = run::run(command, ELEVATION_TIMEOUT)
            .map_err(|error| NodeError::Service(error.to_string()))?;
        let text = clip(&format!("{}\n{}", captured.stdout, captured.stderr));
        if captured.success() {
            return Ok(ElevatedOutput { text });
        }
        let code = captured.status.code();
        if declined(os, code, &text) {
            return Err(NodeError::ElevationDenied);
        }
        Err(NodeError::Service(match code {
            Some(code) if text.is_empty() => format!("the command failed (exit code {code})"),
            Some(code) => format!("{text} (exit code {code})"),
            None => text,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os_args(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    #[test]
    fn pkexec_gets_the_program_and_arguments_as_a_vector() {
        let launcher = pkexec_launcher(
            Path::new("/usr/bin/pkexec"),
            Path::new("/data/cores/2.6.4/easytier-cli"),
            &os_args(&["service", "--name", "easytierui-p-a1", "stop"]),
        );
        assert_eq!(launcher.program, Path::new("/usr/bin/pkexec"));
        assert_eq!(
            launcher.args,
            os_args(&[
                "/data/cores/2.6.4/easytier-cli",
                "service",
                "--name",
                "easytierui-p-a1",
                "stop"
            ])
        );
        assert!(launcher.env.is_empty());
    }

    #[test]
    fn osascript_script_is_constant_and_values_follow_the_separator() {
        let weird = "/Users/o'brien/My Data/cli";
        let launcher = osascript_launcher(
            Path::new("/usr/bin/osascript"),
            "Because",
            Path::new(weird),
            &os_args(&["service", "--name", "x; rm -rf /"]),
        );
        let separator = launcher.args.iter().position(|a| a == "--").unwrap();
        let (script, data) = launcher.args.split_at(separator);
        // Script half: only `-e <fixed line>` pairs.
        assert_eq!(script.len(), APPLESCRIPT.len() * 2);
        for pair in script.chunks(2) {
            assert_eq!(pair.first().unwrap(), "-e");
        }
        assert!(
            !script.iter().any(|a| a.to_string_lossy().contains("rm -rf")
                || a.to_string_lossy().contains("o'brien"))
        );
        // Data half: prompt, program, arguments, untouched.
        assert_eq!(
            data,
            os_args(&["--", "Because", weird, "service", "--name", "x; rm -rf /"])
        );
    }

    #[test]
    fn powershell_script_is_constant_and_inputs_travel_in_the_environment() {
        let launcher = powershell_launcher(
            Path::new(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
            Path::new(r"C:\Users\Jo Doe\AppData\Roaming\EasyTierUI\cores\2.6.4\easytier-cli.exe"),
            &os_args(&[
                "service",
                "--name",
                "easytierui-p-a1",
                "install",
                "--",
                "-c",
                r"C:\Users\Jo Doe\AppData\Roaming\EasyTierUI\instances\p-a1\config.toml",
            ]),
        );
        let script_text: Vec<String> = launcher
            .args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(
            !script_text
                .iter()
                .any(|a| a.contains("Jo Doe") || a.contains("p-a1"))
        );
        let get = |name: &str| {
            launcher
                .env
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.to_string_lossy().into_owned())
        };
        assert_eq!(
            get(WINDOWS_PROGRAM_ENV).as_deref(),
            Some(r"C:\Users\Jo Doe\AppData\Roaming\EasyTierUI\cores\2.6.4\easytier-cli.exe")
        );
        assert_eq!(
            get(WINDOWS_ARGS_ENV).as_deref(),
            Some(
                r#"service --name easytierui-p-a1 install -- -c "C:\Users\Jo Doe\AppData\Roaming\EasyTierUI\instances\p-a1\config.toml""#
            )
        );
    }

    #[test]
    fn windows_quoting_round_trips_the_awkward_cases() {
        assert_eq!(windows_quote("plain"), "plain");
        assert_eq!(windows_quote(""), "\"\"");
        assert_eq!(windows_quote("a b"), "\"a b\"");
        assert_eq!(
            windows_quote(r"C:\dir with space\"),
            r#""C:\dir with space\\""#
        );
        assert_eq!(windows_quote(r#"say "hi""#), r#""say \"hi\"""#);
        assert_eq!(windows_quote(r#"a\"b c"#), r#""a\\\"b c""#);
        assert_eq!(windows_quote(r"C:\no\spaces"), r"C:\no\spaces");
    }

    #[test]
    fn a_declined_prompt_is_recognised_per_platform() {
        assert!(declined(HostOs::Linux, Some(126), ""));
        assert!(declined(HostOs::Linux, Some(127), ""));
        assert!(!declined(HostOs::Linux, Some(1), ""));
        assert!(declined(
            HostOs::MacOs,
            Some(1),
            "execution error: User canceled. (-128)"
        ));
        assert!(!declined(
            HostOs::MacOs,
            Some(1),
            "execution error: Service is not installed (1)"
        ));
        assert!(declined(HostOs::Windows, Some(1223), ""));
        assert!(!declined(HostOs::Windows, Some(1), ""));
    }

    #[test]
    fn long_output_is_clipped() {
        let long = "x".repeat(MAX_MESSAGE_CHARS * 2);
        assert_eq!(clip(&long).chars().count(), MAX_MESSAGE_CHARS + 1);
        assert_eq!(clip("  short \n"), "short");
    }
}

//! Running the official programs: one place that builds a [`Command`] (no shell, no inherited `ET_*` option
//! variables, no console window on Windows) and runs short ones to completion under a deadline.

use std::{
    ffi::OsStr,
    io::{self, Read},
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::Duration,
};

use super::argv::{CONFIG_SERVER_ENV, CORE_ENV_PREFIX};
use crate::core::secret::Secret;

/// How often a running short command is checked for exit.
const POLL_INTERVAL: Duration = Duration::from_millis(15);

/// The most output kept from one short command (a peer list of thousands of nodes fits).
const MAX_CAPTURE: u64 = 8 * 1024 * 1024;

/// `CREATE_NO_WINDOW`: the programs are console apps; the GUI must not flash a console for each.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Why a short command produced no result.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    /// The program could not be started.
    #[error("could not start {program}: {source}")]
    Spawn {
        /// The program's path.
        program: String,
        /// The OS error.
        source: io::Error,
    },
    /// It did not finish before the deadline and was killed.
    #[error("{program} did not finish in {seconds} s")]
    TimedOut {
        /// The program's file name.
        program: String,
        /// The deadline.
        seconds: u64,
    },
    /// Waiting for it or reading its output failed.
    #[error("{program}: {source}")]
    Io {
        /// The program's file name.
        program: String,
        /// The OS error.
        source: io::Error,
    },
}

/// What a finished short command printed.
#[derive(Debug)]
pub struct Captured {
    /// How it ended.
    pub status: ExitStatus,
    /// Standard output, lossy UTF-8. Treat as sensitive: `easytier-cli node` prints the whole config.
    pub stdout: String,
    /// Standard error, lossy UTF-8.
    pub stderr: String,
}

impl Captured {
    /// Whether it exited with status 0.
    pub fn success(&self) -> bool {
        self.status.success()
    }
}

fn file_name(program: &Path) -> String {
    program.file_name().map_or_else(
        || "program".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// A command for `program` with `args`: output piped, stdin closed, `ET_*` option variables of the user's
/// environment removed (a stray `ET_CONFIG_SERVER` must never turn a local instance into a managed one), no console
/// window on Windows.
pub fn command<I, S>(program: &Path, args: I) -> Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with(CORE_ENV_PREFIX) {
            command.env_remove(&name);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// Sets the enrollment URL on `command` as the environment variable `easytier-core` reads it from: the one place the
/// URL leaves the process, set on this one child.
pub fn set_config_server(command: &mut Command, url: &Secret) {
    command.env(CONFIG_SERVER_ENV, url.expose());
}

/// Drains `reader` into a lossy string on a thread, keeping at most [`MAX_CAPTURE`] bytes.
pub fn drain<R: Read + Send + 'static>(reader: R) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Err(error) = reader.take(MAX_CAPTURE).read_to_end(&mut bytes) {
            log::debug!("reading a program's output failed: {error}");
        }
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

fn join_output(handle: Option<thread::JoinHandle<String>>) -> String {
    handle
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default()
}

/// Runs `command` (built with [`command`]) to completion, killing it when `timeout` passes.
///
/// # Errors
///
/// [`RunError::Spawn`] when it cannot start, [`RunError::TimedOut`] after the kill, [`RunError::Io`] when waiting
/// fails.
pub fn run(mut command: Command, timeout: Duration) -> Result<Captured, RunError> {
    let program = file_name(Path::new(command.get_program()));
    let mut child = command.spawn().map_err(|source| RunError::Spawn {
        program: program.clone(),
        source,
    })?;
    let stdout = child.stdout.take().map(drain);
    let stderr = child.stderr.take().map(drain);
    let deadline = nsh_core::time::now()
        .checked_add(timeout)
        .unwrap_or_else(nsh_core::time::now);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(source) => {
                kill_and_reap(&mut child);
                return Err(RunError::Io { program, source });
            }
        }
        if nsh_core::time::now() >= deadline {
            kill_and_reap(&mut child);
            return Err(RunError::TimedOut {
                program,
                seconds: timeout.as_secs(),
            });
        }
        thread::sleep(POLL_INTERVAL);
    };
    Ok(Captured {
        status,
        stdout: join_output(stdout),
        stderr: join_output(stderr),
    })
}

/// Kills `child` and waits for it, logging what fails: nothing is left running.
pub fn kill_and_reap(child: &mut Child) {
    if let Err(error) = child.kill() {
        // `InvalidInput` means it had already exited.
        if error.kind() != io::ErrorKind::InvalidInput {
            log::warn!("could not kill a child process: {error}");
        }
    }
    if let Err(error) = child.wait() {
        log::warn!("could not wait for a killed child process: {error}");
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn captures_output_and_status() {
        let mut cmd = command(
            Path::new("/bin/sh"),
            ["-c", "echo out; echo err >&2; exit 3"],
        );
        cmd.env("ET_NOT_FOR_THE_CHILD", "x");
        let captured = run(cmd, Duration::from_secs(5)).unwrap();
        assert_eq!(captured.status.code(), Some(3));
        assert_eq!(captured.stdout.trim(), "out");
        assert_eq!(captured.stderr.trim(), "err");
    }

    #[test]
    fn a_slow_command_is_killed_at_the_deadline() {
        let cmd = command(Path::new("/bin/sh"), ["-c", "sleep 30"]);
        let started = nsh_core::time::now();
        let error = run(cmd, Duration::from_millis(200)).unwrap_err();
        assert!(matches!(error, RunError::TimedOut { .. }));
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn missing_programs_are_spawn_errors() {
        let cmd = command(Path::new("/nonexistent/easytier-core"), ["--version"]);
        assert!(matches!(
            run(cmd, Duration::from_secs(1)),
            Err(RunError::Spawn { .. })
        ));
    }

    #[test]
    fn option_variables_of_the_user_do_not_reach_the_child() {
        // Set on the builder after `command` ran: explicit variables win, inherited ET_* are removed.
        let mut cmd = command(Path::new("/bin/sh"), ["-c", "echo \"[$ET_CONFIG_SERVER]\""]);
        set_config_server(&mut cmd, &Secret::new("udp://x:1/y"));
        let captured = run(cmd, Duration::from_secs(5)).unwrap();
        assert_eq!(captured.stdout.trim(), "[udp://x:1/y]");
        let plain = command(Path::new("/bin/sh"), ["-c", "echo \"[$ET_CONFIG_SERVER]\""]);
        let captured = run(plain, Duration::from_secs(5)).unwrap();
        assert_eq!(captured.stdout.trim(), "[]");
    }
}

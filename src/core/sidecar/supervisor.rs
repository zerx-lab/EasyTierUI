//! Owning one `easytier-core` child: a supervisor thread captures its output into a bounded, redacted ring, observes
//! its exit and restarts it with capped exponential backoff while the user wants it up. Stopping (or dropping) the
//! supervisor kills the child and waits for it, so no orphan is left behind.

use std::{
    collections::VecDeque,
    ffi::OsString,
    io::{BufRead as _, BufReader, Read},
    path::PathBuf,
    process::{Child, ExitStatus},
    sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError},
    thread::{self, JoinHandle},
    time::Duration,
};

use nsh_core::time::{Instant, now, unix_ms};

use super::{redact::Redactor, run};
use crate::core::{
    secret::Secret,
    state::{FailReason, LogLine, LogStream},
};

/// Lines kept per instance.
pub const RING_LINES: usize = 500;

/// Longest line kept whole; longer ones are split.
const MAX_LINE_BYTES: u64 = 8 * 1024;

/// Lines of output quoted in a [`FailReason::Exited`].
const EXIT_QUOTE_LINES: usize = 20;

/// How often the supervisor checks whether the child exited.
const EXIT_POLL: Duration = Duration::from_millis(100);

/// How long a stopping supervisor waits for the output readers after the child is gone.
const READER_GRACE: Duration = Duration::from_secs(2);

/// Restart policy. [`Timing::DEFAULT`] is what the app uses; tests shrink it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// The delay after the first rapid failure; it doubles for each further one.
    pub base_delay: Duration,
    /// The longest delay.
    pub max_delay: Duration,
    /// A run at least this long is "stable": the failure count starts over.
    pub stable_after: Duration,
    /// This many rapid failures in a row make the supervisor give up.
    pub max_rapid_failures: u32,
}

impl Timing {
    /// 1 s, 2 s, 4 s, 8 s between restarts, capped at 30 s; gives up after 5 failures in a row that each ran under
    /// 30 s.
    pub const DEFAULT: Self = Self {
        base_delay: Duration::from_secs(1),
        max_delay: Duration::from_secs(30),
        stable_after: Duration::from_secs(30),
        max_rapid_failures: 5,
    };

    /// The delay before the restart that follows the `failures`-th rapid failure in a row (1-based).
    pub fn delay_after(&self, failures: u32) -> Duration {
        let doublings = failures.saturating_sub(1).min(16);
        self.base_delay
            .saturating_mul(1_u32.checked_shl(doublings).unwrap_or(u32::MAX))
            .min(self.max_delay)
    }
}

/// Locks, ignoring poison: the data is plain bookkeeping that stays valid if a holder panicked.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A bounded ring of redacted output lines.
#[derive(Debug, Default)]
pub struct LogRing {
    lines: VecDeque<LogLine>,
}

impl LogRing {
    /// Appends `line`, dropping the oldest beyond [`RING_LINES`].
    pub fn push(&mut self, line: LogLine) {
        if self.lines.len() >= RING_LINES {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }

    /// Oldest first.
    pub fn snapshot(&self) -> Vec<LogLine> {
        self.lines.iter().cloned().collect()
    }

    /// The last `count` lines as one text.
    pub fn quote(&self, count: usize) -> String {
        let skip = self.lines.len().saturating_sub(count);
        self.lines
            .iter()
            .skip(skip)
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Whether any of the last `count` lines passes `test`.
    pub fn recent_any(&self, count: usize, test: impl Fn(&str) -> bool) -> bool {
        let skip = self.lines.len().saturating_sub(count);
        self.lines.iter().skip(skip).any(|line| test(&line.text))
    }
}

/// Whether a core output line says creating the network interface was refused for lack of privileges.
pub fn is_tun_permission_error(line: &str) -> bool {
    let lower = line.to_lowercase();
    let tun = [
        "utun",
        "wintun",
        "/dev/net/tun",
        "tun device",
        "tun interface",
        "tun adapter",
        "tun dev",
        "create tun",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    let refused = [
        "permission denied",
        "operation not permitted",
        "access is denied",
        "not permitted",
        "requires root",
        "administrator",
        "os error 1)",
        "os error 13",
        "eperm",
        "eacces",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    tun && refused
}

/// What the supervised child is doing.
#[derive(Debug, Clone, PartialEq)]
pub enum Phase {
    /// The process is alive; since when.
    Up {
        /// When it was spawned.
        since: Instant,
    },
    /// It exited and the supervisor will start it again.
    Backoff {
        /// The exit code, if the OS reported one.
        code: Option<i32>,
        /// Its last lines, redacted.
        last_log: String,
    },
    /// The supervisor stopped trying; the user must act.
    GaveUp(FailReason),
    /// Stopped on request.
    Done,
}

/// What to run.
#[derive(Debug, Clone)]
pub struct Spec {
    /// The `easytier-core` binary.
    pub program: PathBuf,
    /// Its arguments.
    pub args: Vec<OsString>,
    /// The enrollment URL of a managed instance, set as an environment variable on the child only.
    pub config_server: Option<Secret>,
    /// The child's working directory.
    pub work_dir: PathBuf,
}

#[derive(Debug)]
struct Shared {
    stop: Mutex<bool>,
    wake: Condvar,
    ring: Arc<Mutex<LogRing>>,
    phase: Mutex<Phase>,
    redactor: Arc<Redactor>,
}

impl Shared {
    fn app_line(&self, text: impl Into<String>) {
        let text = self.redactor.redact(&text.into());
        lock(&self.ring).push(LogLine {
            at_ms: unix_ms(),
            stream: LogStream::App,
            text,
        });
    }

    fn set_phase(&self, phase: Phase) {
        *lock(&self.phase) = phase;
    }

    fn stop_requested(&self) -> bool {
        *lock(&self.stop)
    }

    /// Sleeps up to `duration` or until a stop is requested; whether it was.
    fn sleep_unless_stopped(&self, duration: Duration) -> bool {
        let deadline = now().checked_add(duration);
        let mut stop = lock(&self.stop);
        loop {
            if *stop {
                return true;
            }
            let remaining = deadline.map_or(Duration::ZERO, |d| d.saturating_duration_since(now()));
            if remaining.is_zero() {
                return false;
            }
            let (guard, _) = self
                .wake
                .wait_timeout(stop, remaining)
                .unwrap_or_else(PoisonError::into_inner);
            stop = guard;
        }
    }
}

/// The supervisor of one child. Dropping it stops the child.
#[derive(Debug)]
pub struct Supervisor {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Supervisor {
    /// Spawns the core and supervises it. A program that cannot be spawned fails here (a missing binary does not heal
    /// by waiting); later restarts that cannot spawn end in [`Phase::GaveUp`].
    ///
    /// # Errors
    ///
    /// The OS error when the process or the supervisor thread cannot be created.
    pub fn start(spec: Spec, redactor: Redactor, timing: Timing) -> std::io::Result<Self> {
        let first = spawn_child(&spec)?;
        let shared = Arc::new(Shared {
            stop: Mutex::new(false),
            wake: Condvar::new(),
            ring: Arc::new(Mutex::new(LogRing::default())),
            phase: Mutex::new(Phase::Up { since: now() }),
            redactor: Arc::new(redactor),
        });
        let worker = Arc::clone(&shared);
        // The child waits in a slot until the thread takes it, so a thread that cannot be created does not strand it.
        let slot = Arc::new(Mutex::new(Some(first)));
        let handoff = Arc::clone(&slot);
        let spawned = thread::Builder::new()
            .name("etui-core-supervisor".to_owned())
            .spawn(move || {
                if let Some(first) = lock(&handoff).take() {
                    supervise(&worker, &spec, timing, first);
                }
            });
        let thread = match spawned {
            Ok(thread) => thread,
            Err(error) => {
                if let Some(mut stranded) = lock(&slot).take() {
                    run::kill_and_reap(&mut stranded);
                }
                return Err(error);
            }
        };
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    /// What the child is doing now.
    pub fn phase(&self) -> Phase {
        lock(&self.shared.phase).clone()
    }

    /// The redacted output so far, oldest first.
    pub fn logs(&self) -> Vec<LogLine> {
        lock(&self.shared.ring).snapshot()
    }

    /// Asks the child to stop without waiting; follow with [`Supervisor::stop`] (or drop) to wait for it.
    pub fn request_stop(&self) {
        *lock(&self.shared.stop) = true;
        self.shared.wake.notify_all();
    }

    /// Kills the child and waits until it is gone.
    pub fn stop(mut self) {
        self.stop_in_place();
    }

    fn stop_in_place(&mut self) {
        self.request_stop();
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            log::error!("a core supervisor thread panicked");
        }
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        self.stop_in_place();
    }
}

/// Spawns the core once: working directory set, enrollment URL in the environment of this child only.
fn spawn_child(spec: &Spec) -> std::io::Result<Child> {
    let mut command = run::command(&spec.program, &spec.args);
    command.current_dir(&spec.work_dir);
    if let Some(url) = &spec.config_server {
        run::set_config_server(&mut command, url);
    }
    command.spawn()
}

/// The supervisor thread's body; `first` is the child [`Supervisor::start`] already spawned.
fn supervise(shared: &Shared, spec: &Spec, timing: Timing, first: Child) {
    let mut rapid_failures = 0_u32;
    let mut next = Some(first);
    loop {
        if shared.stop_requested() {
            if let Some(mut unused) = next.take() {
                run::kill_and_reap(&mut unused);
            }
            shared.set_phase(Phase::Done);
            return;
        }
        let started = now();
        let mut child = match next.take() {
            Some(child) => child,
            None => match spawn_child(spec) {
                Ok(child) => child,
                Err(error) => {
                    shared.app_line(format!("the core could not be started: {error}"));
                    shared.set_phase(Phase::GaveUp(FailReason::Other(format!(
                        "the core could not be started: {error}"
                    ))));
                    return;
                }
            },
        };
        shared.set_phase(Phase::Up { since: started });
        shared.app_line("the core started");
        let readers = capture_output(shared, &mut child);
        let exit = wait_for_exit(shared, &mut child);
        join_readers(readers);
        let status = match exit {
            Exit::Requested => {
                shared.app_line("the core was stopped");
                shared.set_phase(Phase::Done);
                return;
            }
            Exit::Ended(status) => status,
        };
        let code = status.and_then(|status| status.code());
        let last_log = lock(&shared.ring).quote(EXIT_QUOTE_LINES);
        shared.app_line(match code {
            Some(code) => format!("the core exited with code {code}"),
            None => "the core was terminated by a signal".to_owned(),
        });
        if lock(&shared.ring).recent_any(EXIT_QUOTE_LINES, is_tun_permission_error) {
            shared.set_phase(Phase::GaveUp(FailReason::NoPrivileges));
            return;
        }
        if started.elapsed() >= timing.stable_after {
            rapid_failures = 0;
        }
        rapid_failures = rapid_failures.saturating_add(1);
        if rapid_failures >= timing.max_rapid_failures {
            shared.app_line("the core keeps exiting; not restarting it again");
            shared.set_phase(Phase::GaveUp(FailReason::Exited { code, last_log }));
            return;
        }
        let delay = timing.delay_after(rapid_failures);
        shared.set_phase(Phase::Backoff { code, last_log });
        shared.app_line(format!(
            "restarting the core in {} s",
            delay.as_secs().max(1)
        ));
        if shared.sleep_unless_stopped(delay) {
            shared.set_phase(Phase::Done);
            return;
        }
    }
}

enum Exit {
    /// The user asked it to stop; the child was killed.
    Requested,
    /// It ended on its own (`None`: waiting for it failed).
    Ended(Option<ExitStatus>),
}

fn wait_for_exit(shared: &Shared, child: &mut Child) -> Exit {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Exit::Ended(Some(status)),
            Ok(None) => {}
            Err(error) => {
                log::warn!("waiting for the core failed: {error}");
                run::kill_and_reap(child);
                return Exit::Ended(None);
            }
        }
        let stop = lock(&shared.stop);
        if *stop {
            drop(stop);
            run::kill_and_reap(child);
            return Exit::Requested;
        }
        let waited = shared.wake.wait_timeout(stop, EXIT_POLL);
        drop(waited.unwrap_or_else(PoisonError::into_inner));
    }
}

fn capture_output(shared: &Shared, child: &mut Child) -> Vec<JoinHandle<()>> {
    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        readers.push(reader_thread(shared, stdout, LogStream::Stdout));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(reader_thread(shared, stderr, LogStream::Stderr));
    }
    readers.into_iter().flatten().collect()
}

/// Copies lines of `source` into the ring; the thread owns shared handles to the ring and the redactor, so a slow
/// reader may outlive a stop.
fn reader_thread<R: Read + Send + 'static>(
    shared: &Shared,
    source: R,
    stream: LogStream,
) -> Option<JoinHandle<()>> {
    let sink = ReaderSink::of(shared);
    thread::Builder::new()
        .name("etui-core-output".to_owned())
        .spawn(move || {
            let mut reader = BufReader::new(source);
            let mut buffer = Vec::new();
            loop {
                buffer.clear();
                match (&mut reader)
                    .take(MAX_LINE_BYTES)
                    .read_until(b'\n', &mut buffer)
                {
                    Ok(0) => return,
                    Ok(_) => sink.push(stream, &buffer),
                    Err(error) => {
                        log::debug!("reading the core's output ended: {error}");
                        return;
                    }
                }
            }
        })
        .map_err(|error| log::warn!("could not start an output reader: {error}"))
        .ok()
}

/// The parts of [`Shared`] a reader thread writes to (shared ownership, so the thread may outlive a stop).
struct ReaderSink {
    ring: Arc<Mutex<LogRing>>,
    redactor: Arc<Redactor>,
}

impl ReaderSink {
    fn of(shared: &Shared) -> Self {
        Self {
            ring: Arc::clone(&shared.ring),
            redactor: Arc::clone(&shared.redactor),
        }
    }

    fn push(&self, stream: LogStream, raw: &[u8]) {
        let text = String::from_utf8_lossy(raw);
        let line = text.trim_end_matches(['\n', '\r']);
        if line.trim().is_empty() {
            return;
        }
        lock(&self.ring).push(LogLine {
            at_ms: unix_ms(),
            stream,
            text: self.redactor.redact(line),
        });
    }
}

/// Waits (bounded) for the output readers, which end when the child's pipes close.
fn join_readers(readers: Vec<JoinHandle<()>>) {
    let deadline = now().checked_add(READER_GRACE);
    for reader in readers {
        while !reader.is_finished() && deadline.is_some_and(|d| now() < d) {
            thread::sleep(Duration::from_millis(5));
        }
        if reader.is_finished() {
            if reader.join().is_err() {
                log::warn!("an output reader panicked");
            }
        } else {
            log::debug!("an output reader is still open; leaving it to end with the pipe");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_backoff_schedule_doubles_and_is_capped() {
        let timing = Timing::DEFAULT;
        let secs: Vec<u64> = (1..=8).map(|n| timing.delay_after(n).as_secs()).collect();
        assert_eq!(secs, [1, 2, 4, 8, 16, 30, 30, 30]);
        assert_eq!(timing.delay_after(0).as_secs(), 1);
        assert_eq!(timing.delay_after(u32::MAX).as_secs(), 30);
    }

    #[test]
    fn the_ring_is_bounded_and_quotes_the_tail() {
        let mut ring = LogRing::default();
        for n in 0..(RING_LINES + 25) {
            ring.push(LogLine {
                at_ms: 0.0,
                stream: LogStream::Stdout,
                text: format!("line {n}"),
            });
        }
        let lines = ring.snapshot();
        assert_eq!(lines.len(), RING_LINES);
        assert_eq!(lines.first().unwrap().text, "line 25");
        assert_eq!(
            ring.quote(2),
            format!("line {}\nline {}", RING_LINES + 23, RING_LINES + 24)
        );
        assert!(ring.recent_any(1, |t| t.ends_with(&format!("{}", RING_LINES + 24))));
        assert!(!ring.recent_any(1, |t| t == "line 25"));
    }

    #[test]
    fn tun_permission_errors_are_recognised() {
        assert!(is_tun_permission_error(
            "failed to create tun device: Operation not permitted (os error 1)"
        ));
        assert!(is_tun_permission_error("Error: utun: Permission denied"));
        assert!(!is_tun_permission_error("connection refused by 10.0.0.1"));
        assert!(!is_tun_permission_error(
            "permission denied reading /etc/hosts"
        ));
    }

    #[cfg(unix)]
    mod with_processes {
        use super::*;

        fn fast() -> Timing {
            Timing {
                base_delay: Duration::from_millis(20),
                max_delay: Duration::from_millis(80),
                stable_after: Duration::from_secs(60),
                max_rapid_failures: 3,
            }
        }

        fn shell(script: &str) -> Spec {
            Spec {
                program: PathBuf::from("/bin/sh"),
                args: vec![OsString::from("-c"), OsString::from(script)],
                config_server: None,
                work_dir: std::env::temp_dir(),
            }
        }

        fn wait_for(supervisor: &Supervisor, done: impl Fn(&Phase) -> bool) -> Phase {
            for _ in 0..400 {
                let phase = supervisor.phase();
                if done(&phase) {
                    return phase;
                }
                thread::sleep(Duration::from_millis(25));
            }
            supervisor.phase()
        }

        #[test]
        fn output_is_captured_redacted_and_a_failing_child_is_given_up_on() {
            let script =
                "echo 'hello'; echo 'network_secret = \"example-secret-do-not-use\"' >&2; exit 7";
            let redactor = Redactor::new().with_secret("example-secret-do-not-use");
            let supervisor = Supervisor::start(shell(script), redactor, fast()).unwrap();
            let phase = wait_for(&supervisor, |p| matches!(p, Phase::GaveUp(_)));
            let Phase::GaveUp(FailReason::Exited { code, last_log }) = phase else {
                panic!("expected a give-up, got {phase:?}");
            };
            assert_eq!(code, Some(7));
            assert!(last_log.contains("hello"));
            let logs = supervisor.logs();
            assert!(
                logs.iter()
                    .any(|l| l.text == "hello" && l.stream == LogStream::Stdout)
            );
            assert!(
                logs.iter().all(|l| !l.text.contains("example-secret")),
                "{logs:?}"
            );
            assert!(
                logs.iter()
                    .any(|l| l.stream == LogStream::App && l.text.contains("exited with code 7"))
            );
            // Three runs, each announced.
            assert_eq!(
                logs.iter().filter(|l| l.text == "the core started").count(),
                3
            );
        }

        #[test]
        fn a_missing_program_fails_to_start_instead_of_being_retried() {
            let spec = Spec {
                program: PathBuf::from("/nonexistent/easytier-core"),
                ..shell("true")
            };
            assert!(Supervisor::start(spec, Redactor::new(), fast()).is_err());
        }

        #[test]
        fn stopping_kills_a_long_running_child_and_waits() {
            let supervisor =
                Supervisor::start(shell("echo up; sleep 60"), Redactor::new(), fast()).unwrap();
            let phase = wait_for(&supervisor, |p| matches!(p, Phase::Up { .. }));
            assert!(matches!(phase, Phase::Up { .. }));
            let started = now();
            supervisor.stop();
            assert!(started.elapsed() < Duration::from_secs(5));
        }

        #[test]
        fn a_stop_during_backoff_ends_the_supervisor_promptly() {
            let timing = Timing {
                base_delay: Duration::from_secs(30),
                ..fast()
            };
            let supervisor = Supervisor::start(shell("exit 1"), Redactor::new(), timing).unwrap();
            let phase = wait_for(&supervisor, |p| matches!(p, Phase::Backoff { .. }));
            assert!(
                matches!(phase, Phase::Backoff { code: Some(1), .. }),
                "{phase:?}"
            );
            let started = now();
            supervisor.stop();
            assert!(started.elapsed() < Duration::from_secs(5));
        }

        #[test]
        fn the_config_server_reaches_only_the_environment_of_the_child() {
            let spec = Spec {
                config_server: Some(Secret::new(
                    "udp://config.example:22020/example-user-do-not-use",
                )),
                ..shell("echo \"[$ET_CONFIG_SERVER]\"; sleep 60")
            };
            let redactor =
                Redactor::new().with_secret("udp://config.example:22020/example-user-do-not-use");
            let supervisor = Supervisor::start(spec, redactor, fast()).unwrap();
            wait_for(&supervisor, |_| {
                supervisor.logs().iter().any(|l| l.text.starts_with('['))
            });
            let logs = supervisor.logs();
            // The child saw the URL (it echoed it); the ring only ever holds the redacted form.
            assert!(logs.iter().any(|l| l.text == "[***]"), "{logs:?}");
            supervisor.stop();
        }
    }
}

//! The OS-service half of the sidecar: `easytier-cli service …` run with administrator rights (install, start, stop,
//! uninstall) or without (status), tolerant of the "already in that state" answers a restart naturally produces.

use std::{ffi::OsString, path::Path, time::Duration};

use super::{
    argv::{self, ServiceVerb},
    elevate::Elevate,
    redact::Redactor,
    run,
};
use crate::core::node::NodeError;

/// How long an unprivileged `service status` may take.
const STATUS_TIMEOUT: Duration = Duration::from_secs(5);

/// What the service manager says about the service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    /// Registered and running.
    Running,
    /// Registered, not running.
    Stopped,
    /// Not registered.
    NotInstalled,
}

/// Reads `easytier-cli service status`'s one-line answer.
pub fn parse_status(stdout: &str) -> Option<ServiceState> {
    let lower = stdout.to_lowercase();
    if lower.contains("not installed") {
        Some(ServiceState::NotInstalled)
    } else if lower.contains("is running") {
        Some(ServiceState::Running)
    } else if lower.contains("is stopped") {
        Some(ServiceState::Stopped)
    } else {
        None
    }
}

/// The service's state without asking for rights; `None` when the service manager will not tell an ordinary user
/// (macOS' `launchctl` for system daemons, a missing manager) or the CLI fails.
pub fn query_status(cli: &Path, service: &str) -> Option<ServiceState> {
    let command = run::command(cli, argv::service_verb(service, ServiceVerb::Status));
    match run::run(command, STATUS_TIMEOUT) {
        Ok(captured) if captured.success() => parse_status(&captured.stdout),
        Ok(_) => None,
        Err(error) => {
            log::debug!("the service status could not be read: {error}");
            None
        }
    }
}

/// How an elevated `service` command's failure is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnFailure {
    /// Any failure but a declined prompt is fine (a best-effort stop before a restart).
    Ignore,
    /// Fine when the message says the service is already in the wanted state (one of these phrases, lower case).
    Tolerate(&'static [&'static str]),
    /// Every failure is an error.
    Fail,
}

/// `start` after the service is registered.
pub const START_TOLERATES: &[&str] = &["already running"];
/// `stop` of a service that may not be running.
pub const STOP_TOLERATES: &[&str] = &["already stopped", "not installed"];

/// Runs `cli args…` elevated.
///
/// # Errors
///
/// [`NodeError::ElevationDenied`] whenever the prompt was declined; otherwise per `on_failure`, a
/// [`NodeError::Service`] whose message is redacted.
pub fn run_elevated(
    elevator: &dyn Elevate,
    cli: &Path,
    args: &[OsString],
    on_failure: OnFailure,
) -> Result<(), NodeError> {
    match elevator.run(cli, args) {
        Ok(_) => Ok(()),
        Err(NodeError::Service(message)) => {
            let lower = message.to_lowercase();
            let fine = match on_failure {
                OnFailure::Ignore => true,
                OnFailure::Tolerate(phrases) => phrases.iter().any(|phrase| lower.contains(phrase)),
                OnFailure::Fail => false,
            };
            if fine {
                log::debug!("an elevated service command ended with a tolerated answer");
                Ok(())
            } else {
                Err(NodeError::Service(Redactor::new().redact(&message)))
            }
        }
        Err(other) => Err(other),
    }
}

#[cfg(test)]
pub mod testing {
    //! A recording [`Elevate`] for tests: nothing is elevated, nothing is run.

    use std::{
        ffi::OsString,
        path::{Path, PathBuf},
        sync::Mutex,
    };

    use super::super::elevate::{Elevate, ElevatedOutput};
    use crate::core::node::NodeError;

    /// Records every call; answers from a script.
    #[derive(Default)]
    pub struct Recorder {
        /// `(program, args)` of each call, in order.
        pub calls: Mutex<Vec<(PathBuf, Vec<String>)>>,
        /// Answers popped in order; an empty script answers success.
        pub script: Mutex<Vec<Result<(), NodeError>>>,
    }

    impl Recorder {
        /// The recorded argument lists, joined by spaces.
        pub fn lines(&self) -> Vec<String> {
            self.calls
                .lock()
                .map(|calls| calls.iter().map(|(_, args)| args.join(" ")).collect())
                .unwrap_or_default()
        }
    }

    impl Elevate for Recorder {
        fn run(&self, program: &Path, args: &[OsString]) -> Result<ElevatedOutput, NodeError> {
            if let Ok(mut calls) = self.calls.lock() {
                calls.push((
                    program.to_path_buf(),
                    args.iter()
                        .map(|a| a.to_string_lossy().into_owned())
                        .collect(),
                ));
            }
            let next = self.script.lock().ok().and_then(|mut script| {
                if script.is_empty() {
                    None
                } else {
                    Some(script.remove(0))
                }
            });
            match next {
                Some(Err(error)) => Err(error),
                _ => Ok(ElevatedOutput {
                    text: String::new(),
                }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{testing::Recorder, *};

    #[test]
    fn status_lines_are_read() {
        assert_eq!(
            parse_status("Service is running\n"),
            Some(ServiceState::Running)
        );
        assert_eq!(
            parse_status("Service is stopped"),
            Some(ServiceState::Stopped)
        );
        assert_eq!(
            parse_status("Service is not installed"),
            Some(ServiceState::NotInstalled)
        );
        assert_eq!(parse_status("something else"), None);
    }

    fn failing(message: &str) -> Recorder {
        let recorder = Recorder::default();
        recorder
            .script
            .lock()
            .unwrap()
            .push(Err(NodeError::Service(message.to_owned())));
        recorder
    }

    #[test]
    fn tolerated_answers_are_success_and_others_are_errors() {
        let cli = Path::new("/c/easytier-cli");
        let args = argv::service_verb("easytierui-p-a1", ServiceVerb::Start);
        assert!(
            run_elevated(
                &failing("Error: Service is already running (exit code 1)"),
                cli,
                &args,
                OnFailure::Tolerate(START_TOLERATES)
            )
            .is_ok()
        );
        assert!(matches!(
            run_elevated(
                &failing("failed to start service: access denied"),
                cli,
                &args,
                OnFailure::Tolerate(START_TOLERATES)
            ),
            Err(NodeError::Service(_))
        ));
        assert!(run_elevated(&failing("anything"), cli, &args, OnFailure::Ignore).is_ok());
        assert!(run_elevated(&failing("anything"), cli, &args, OnFailure::Fail).is_err());
    }

    #[test]
    fn a_declined_prompt_is_never_swallowed() {
        let recorder = Recorder::default();
        recorder
            .script
            .lock()
            .unwrap()
            .push(Err(NodeError::ElevationDenied));
        let result = run_elevated(
            &recorder,
            Path::new("/c/easytier-cli"),
            &argv::service_verb("s", ServiceVerb::Stop),
            OnFailure::Ignore,
        );
        assert_eq!(result, Err(NodeError::ElevationDenied));
    }

    #[test]
    fn error_messages_are_redacted() {
        let result = run_elevated(
            &failing("boom network_secret = hunter2-do-not-use"),
            Path::new("/c/easytier-cli"),
            &[],
            OnFailure::Fail,
        );
        let Err(NodeError::Service(message)) = result else {
            panic!("expected a service error");
        };
        assert!(!message.contains("hunter2"), "{message}");
    }
}

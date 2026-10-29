//! Blocking work (process spawns, CLI reads, file writes, library calls) off the engine thread, awaited from an
//! NSH task (`spawn`) so its result can be written into signals there.

/// Why off-thread work produced no result.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorkError {
    /// The worker panicked or was cancelled; the message is the runtime's.
    #[error("the background task ended without a result: {0}")]
    Lost(String),
    /// The platform has no threads for blocking work (the web build).
    #[error("this platform cannot run blocking work")]
    Unavailable,
}

/// Runs `f` on the runtime's blocking threads and waits for its result in the calling task.
///
/// # Errors
///
/// [`WorkError::Lost`] when the worker did not return.
#[cfg(not(target_arch = "wasm32"))]
pub async fn off_thread<R: Send + 'static>(
    f: impl FnOnce() -> R + Send + 'static,
) -> Result<R, WorkError> {
    nsh_core::prelude::blocking(f)
        .await
        .map_err(|error| WorkError::Lost(error.to_string()))
}

/// The web build has no threads; nothing that blocks runs there (its node and host features are remote).
///
/// # Errors
///
/// Always [`WorkError::Unavailable`].
#[cfg(target_arch = "wasm32")]
pub async fn off_thread<R: Send + 'static>(
    _f: impl FnOnce() -> R + Send + 'static,
) -> Result<R, WorkError> {
    Err(WorkError::Unavailable)
}

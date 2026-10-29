//! What can go wrong attaching to the window, making the tray icon or claiming the single instance.

use std::{io, path::PathBuf};

/// A failure of this crate's shell, tray or instance functions.
#[derive(Debug, thiserror::Error)]
pub enum ShellError {
    /// The process has no NSH window on this thread: a headless run (tests), or the UI thread is another one.
    #[error("this process has no app window to attach to")]
    NoWindow,
    /// The tray icon or its menu could not be created.
    #[error("the tray icon could not be created: {0}")]
    Tray(String),
    /// The tray icon's image is not an 8-bit RGBA PNG.
    #[error("the tray icon's image is not an RGBA PNG: {0}")]
    Icon(String),
    /// A file of the single-instance lock could not be used.
    #[error("{action} {}: {source}", path.display())]
    Io {
        /// What was being done ("open", "write", ...).
        action: &'static str,
        /// The file.
        path: PathBuf,
        /// The OS error.
        #[source]
        source: io::Error,
    },
    /// The single-instance listener could not be set up or reached.
    #[error("the instance listener: {0}")]
    Listener(#[source] io::Error),
}

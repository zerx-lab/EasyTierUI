//! Captures EasyTier's `tracing` output into a bounded ring buffer a host app can read.
//!
//! EasyTier logs through `tracing`; on a phone nobody reads stderr, so the bridge installs one global subscriber
//! that keeps the newest lines (info and above). Every line is redacted (see `redact`) before it is stored.
//! The buffer is shared by all instances: tracing events carry no instance identity.

use std::{
    collections::VecDeque,
    fmt::Debug,
    sync::{LazyLock, Mutex, PoisonError},
    time::{SystemTime, UNIX_EPOCH},
};

use tracing::{
    Event, Level, Subscriber,
    field::{Field, Visit},
};
use tracing_subscriber::{
    Layer,
    filter::LevelFilter,
    layer::{Context, SubscriberExt},
};

use crate::redact;

/// How many lines the buffer keeps; older lines are dropped first.
const CAPACITY: usize = 500;

/// A log line's severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum LogLevel {
    /// Very detailed tracing (never captured by default).
    Trace,
    /// Debugging detail (never captured by default).
    Debug,
    /// Normal operation: a listener bound, a peer connected.
    Info,
    /// Something is wrong but the instance continues.
    Warn,
    /// An operation failed.
    Error,
}

/// One captured log line, already redacted.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct LogEntry {
    /// When it was logged, in Unix milliseconds.
    pub at_ms: u64,
    /// Its severity.
    pub level: LogLevel,
    /// The module that logged it (for example `easytier::peers::peer_conn`).
    pub target: String,
    /// The message with the event's fields appended as `name=value`.
    pub message: String,
}

static BUFFER: Mutex<VecDeque<LogEntry>> = Mutex::new(VecDeque::new());

/// Installs the global subscriber on first use; whether it took effect.
static INSTALLED: LazyLock<bool> = LazyLock::new(|| {
    let subscriber =
        tracing_subscriber::registry().with(CaptureLayer.with_filter(LevelFilter::INFO));
    tracing::subscriber::set_global_default(subscriber).is_ok()
});

/// Milliseconds since the Unix epoch; 0 when the clock is before it.
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| u64::try_from(elapsed.as_millis()).ok())
        .unwrap_or(0)
}

fn level_of(level: &Level) -> LogLevel {
    match *level {
        Level::TRACE => LogLevel::Trace,
        Level::DEBUG => LogLevel::Debug,
        Level::INFO => LogLevel::Info,
        Level::WARN => LogLevel::Warn,
        Level::ERROR => LogLevel::Error,
    }
}

/// Appends one line, evicting the oldest when full.
pub(crate) fn push(level: LogLevel, target: &str, message: &str) {
    let entry = LogEntry {
        at_ms: now_ms(),
        level,
        target: target.to_owned(),
        message: redact::scrub(message),
    };
    let mut buffer = BUFFER.lock().unwrap_or_else(PoisonError::into_inner);
    if buffer.len() >= CAPACITY {
        buffer.pop_front();
    }
    buffer.push_back(entry);
}

/// The buffered lines, oldest first.
pub(crate) fn snapshot() -> Vec<LogEntry> {
    BUFFER
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .cloned()
        .collect()
}

/// Collects an event's `message` and fields into one line.
#[derive(Default)]
struct LineVisitor {
    message: String,
    fields: String,
}

impl Visit for LineVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        } else {
            self.fields
                .push_str(&format!(" {}={value:?}", field.name()));
        }
    }
}

/// The layer that feeds the buffer.
struct CaptureLayer;

impl<S: Subscriber> Layer<S> for CaptureLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = LineVisitor::default();
        event.record(&mut visitor);
        let line = format!("{}{}", visitor.message, visitor.fields);
        push(
            level_of(event.metadata().level()),
            event.metadata().target(),
            &line,
        );
    }
}

/// Installs the global subscriber once. Returns whether the capture is active: another subscriber installed by
/// the host app first wins, and then the buffer only holds the bridge's own lines.
pub(crate) fn install() -> bool {
    *INSTALLED
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_buffer_keeps_the_newest_lines_and_masks_secrets() {
        redact::register("p-logs-test", vec!["example-secret-do-not-use".to_owned()]);
        for index in 0..(CAPACITY + 10) {
            push(LogLevel::Info, "test", &format!("line {index}"));
        }
        push(
            LogLevel::Warn,
            "test",
            "secret example-secret-do-not-use leaked",
        );
        let lines = snapshot();
        assert!(lines.len() <= CAPACITY);
        let last = lines.last().unwrap();
        assert_eq!(last.message, "secret *** leaked");
        assert_eq!(last.level, LogLevel::Warn);
        assert!(lines.iter().all(|line| line.message != "line 0"));
        redact::forget("p-logs-test");
    }
}

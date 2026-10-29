//! Text for the console pages: sizes, times, server names and the words for a failed request.

use crate::{
    core::{
        remote::RemoteError,
        target::{Target, TargetKind, TargetOrigin},
    },
    i18n::{Strings, fill},
};

/// A byte count as `1.4 MiB`.
pub(super) fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit: usize = 0;
    while value >= 1024.0 && unit < UNITS.len().saturating_sub(1) {
        value /= 1024.0;
        unit = unit.saturating_add(1);
    }
    let name = UNITS.get(unit).copied().unwrap_or("B");
    format!("{value:.1} {name}")
}

/// A round-trip time as `23 ms`.
pub(super) fn format_latency(ms: f64) -> String {
    format!("{ms:.0} ms")
}

/// A packet loss fraction as `0.5 %`.
pub(super) fn format_loss(fraction: f64) -> String {
    format!("{:.1} %", fraction * 100.0)
}

/// How long ago `then_ms` was at `now_ms` (both Unix milliseconds), in the app's words: `just now`, `12 s ago`,
/// `3 min ago`, `5 h ago`, `2 d ago`. A time in the future counts as now.
pub(super) fn format_ago(strings: &'static Strings, now_ms: f64, then_ms: f64) -> String {
    let c = strings.console;
    let seconds = ((now_ms - then_ms) / 1000.0).floor().max(0.0);
    let (template, amount) = if seconds < 1.0 {
        return c.ago_now.to_owned();
    } else if seconds < 120.0 {
        (c.ago_seconds, seconds)
    } else if seconds < 7200.0 {
        (c.ago_minutes, (seconds / 60.0).floor())
    } else if seconds < 172_800.0 {
        (c.ago_hours, (seconds / 3600.0).floor())
    } else {
        (c.ago_days, (seconds / 86_400.0).floor())
    };
    fill(template, &[("age", format!("{amount:.0}").as_str())])
}

/// A Unix time in milliseconds as `2026-10-30 12:00 UTC`.
pub(super) fn format_utc(ms: i64) -> String {
    let seconds = ms.div_euclid(1000);
    let days = seconds.div_euclid(86_400);
    let of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = of_day.div_euclid(3600);
    let minute = of_day.rem_euclid(3600).div_euclid(60);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02} UTC")
}

/// The proleptic Gregorian date of `days` since 1970-01-01 (Howard Hinnant's `civil_from_days`).
#[expect(
    clippy::arithmetic_side_effects,
    reason = "calendar arithmetic on values bounded by the euclidean division that precedes it"
)]
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days.saturating_add(719_468);
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// A server's name: the one the user gave it, else its host, else what it is.
pub(super) fn server_name(target: &Target, strings: &'static Strings) -> String {
    if let Some(name) = &target.name {
        return name.to_string();
    }
    match &target.origin {
        TargetOrigin::Local => strings.target_this_device.to_owned(),
        TargetOrigin::SameOrigin => strings.target_this_server.to_owned(),
        TargetOrigin::Loopback { .. } => strings.target_hosted.to_owned(),
        TargetOrigin::Url(url) => url::Url::parse(url)
            .ok()
            .and_then(|parsed| parsed.host_str().map(str::to_owned))
            .unwrap_or_else(|| url.clone()),
    }
}

/// A server's address, for machine text; the serving origin has none to show.
pub(super) fn server_address(target: &Target) -> Option<String> {
    match &target.origin {
        TargetOrigin::Url(url) => Some(url.clone()),
        TargetOrigin::Loopback { port } => Some(format!("http://127.0.0.1:{port}")),
        TargetOrigin::SameOrigin | TargetOrigin::Local => None,
    }
}

/// Whether the user can remove this target (a server they added).
pub(super) fn is_removable(target: &Target) -> bool {
    target.kind == TargetKind::Server && matches!(target.origin, TargetOrigin::Url(_))
}

/// The words for a failed request: what happened in the app's language, with the server's own message where it
/// gave one.
pub(super) fn remote_error_text(strings: &'static Strings, error: &RemoteError) -> String {
    let c = strings.console;
    match error {
        RemoteError::Unauthorized => c.err_session_ended.to_owned(),
        RemoteError::BadCredentials => c.err_bad_credentials.to_owned(),
        RemoteError::NotFound(_) => c.err_machine_gone.to_owned(),
        RemoteError::Rejected { message, .. } => {
            fill(c.err_rejected, &[("message", message.as_str())])
        }
        RemoteError::Timeout => c.err_timeout.to_owned(),
        RemoteError::Network(message) => fill(c.err_network, &[("message", message.as_str())]),
        RemoteError::Decode(_) => c.err_decode.to_owned(),
        RemoteError::InsecureTransport => c.err_insecure.to_owned(),
        RemoteError::InvalidAddress(_) => c.err_invalid_address.to_owned(),
        RemoteError::Invalid(message) => {
            fill(c.err_invalid_request, &[("message", message.as_str())])
        }
    }
}

//! Machine data as text: byte counts, latency, loss and addresses. Always shown in monospace by the caller.

use ipnet::Ipv4Net;

use crate::components::ChartUnit;

/// Latency past which a peer's figure turns amber, in milliseconds.
pub(super) const SLOW_LATENCY_MS: f64 = 150.0;
/// Loss past which a peer's figure turns amber, as a fraction.
pub(super) const LOSSY: f64 = 0.05;
/// Loss past which a peer's figure turns red, as a fraction.
pub(super) const LOSS_BAD: f64 = 0.10;

/// A byte count in binary units, one decimal past bytes: `0 B`, `1.5 KiB`, `3.2 GiB`.
pub(super) fn bytes_text(bytes: Option<u64>) -> String {
    let Some(bytes) = bytes else {
        return DASH.to_owned();
    };
    const UNITS: [&str; 5] = ["KiB", "MiB", "GiB", "TiB", "PiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    // Whole tenths of the unit, in integers: no float casts of a count.
    let mut tenths = u128::from(bytes).saturating_mul(10);
    let mut unit = 0_usize;
    loop {
        tenths = tenths.checked_div(1024).unwrap_or_default();
        let next = unit.saturating_add(1);
        if tenths < 10_240 || next >= UNITS.len() {
            break;
        }
        unit = next;
    }
    let whole = tenths.checked_div(10).unwrap_or_default();
    let tenth = tenths.checked_rem(10).unwrap_or_default();
    let name = UNITS.get(unit).copied().unwrap_or("PiB");
    format!("{whole}.{tenth} {name}")
}

/// A placeholder for a figure the core did not give.
pub(super) const DASH: &str = "—";

/// A latency: `23 ms`, one decimal under 10 ms.
pub(super) fn latency_text(latency_ms: Option<f64>) -> String {
    match latency_ms {
        None => DASH.to_owned(),
        Some(ms) if ms < 10.0 => format!("{ms:.1} ms"),
        Some(ms) => format!("{ms:.0} ms"),
    }
}

/// Packet loss as a percentage.
pub(super) fn loss_text(loss: Option<f64>) -> String {
    match loss {
        None => DASH.to_owned(),
        Some(fraction) => format!("{:.1}%", fraction.clamp(0.0, 1.0) * 100.0),
    }
}

/// A virtual IP with its prefix, or a dash.
pub(super) fn ip_text(address: Option<Ipv4Net>) -> String {
    address.map_or_else(|| DASH.to_owned(), |net| net.to_string())
}

/// A virtual IP without its prefix; empty (a dash in a table cell) when there is none.
pub(super) fn address_text(address: Option<Ipv4Net>) -> String {
    address.map_or_else(String::new, |net| net.addr().to_string())
}

/// A rate in bytes per second with its unit: `120 B/s`, `1.5 MiB/s`.
pub(super) fn rate_text(bytes_per_second: f64) -> String {
    ChartUnit::BytesPerSecond.format(bytes_per_second)
}

/// A rate in bytes per second as its number and its unit (`1.5`, `MiB/s`), for a summary cell.
pub(super) fn rate_parts(bytes_per_second: f64) -> (String, String) {
    let text = ChartUnit::BytesPerSecond.format(bytes_per_second);
    match text.split_once(' ') {
        Some((number, unit)) => (number.to_owned(), unit.to_owned()),
        None => (text, String::new()),
    }
}

/// The last `max` characters of `text`, with an ellipsis in front when something was cut.
pub(super) fn tail_chars(text: &str, max: usize) -> String {
    let total = text.chars().count();
    if total <= max {
        return text.trim_end().to_owned();
    }
    let skip = total.saturating_sub(max);
    let tail: String = text.chars().skip(skip).collect();
    format!("…{}", tail.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_use_binary_units_with_one_decimal() {
        assert_eq!(bytes_text(Some(0)), "0 B");
        assert_eq!(bytes_text(Some(1023)), "1023 B");
        assert_eq!(bytes_text(Some(1024)), "1.0 KiB");
        assert_eq!(bytes_text(Some(1536)), "1.5 KiB");
        assert_eq!(bytes_text(Some(5 * 1024 * 1024 + 512 * 1024)), "5.5 MiB");
        assert_eq!(bytes_text(Some(3 * 1024 * 1024 * 1024)), "3.0 GiB");
        assert_eq!(bytes_text(None), "—");
        assert!(bytes_text(Some(u64::MAX)).ends_with("PiB"));
    }

    #[test]
    fn latency_and_loss_are_short() {
        assert_eq!(latency_text(Some(23.4)), "23 ms");
        assert_eq!(latency_text(Some(2.71)), "2.7 ms");
        assert_eq!(latency_text(None), "—");
        assert_eq!(loss_text(Some(0.0)), "0.0%");
        assert_eq!(loss_text(Some(0.125)), "12.5%");
        assert_eq!(loss_text(Some(7.0)), "100.0%");
    }

    #[test]
    fn tails_keep_whole_characters() {
        assert_eq!(tail_chars("short\n", 10), "short");
        assert_eq!(tail_chars("日志日志日志", 3), "…志日志");
    }
}

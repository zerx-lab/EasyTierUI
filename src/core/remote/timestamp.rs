//! The timestamps easytier-web reports (`2026-10-29T07:23:40.084490+08:00`), as Unix milliseconds: the unit of
//! `nsh_core::time::unix_ms`, so the UI can show an age without a date library.

/// Unix milliseconds of an RFC 3339 timestamp; `None` when the text is not one.
#[expect(
    clippy::arithmetic_side_effects,
    reason = "the zone offset is range-checked (hours 0..=23, minutes 0..=59) before it is multiplied"
)]
pub(crate) fn parse_rfc3339_ms(text: &str) -> Option<f64> {
    let text = text.trim();
    let year: i64 = text.get(0..4)?.parse().ok()?;
    expect_char(text, 4, "-")?;
    let month: i64 = text.get(5..7)?.parse().ok()?;
    expect_char(text, 7, "-")?;
    let day: i64 = text.get(8..10)?.parse().ok()?;
    if !matches!(text.get(10..11)?, "T" | "t" | " ") {
        return None;
    }
    let hour: i64 = text.get(11..13)?.parse().ok()?;
    expect_char(text, 13, ":")?;
    let minute: i64 = text.get(14..16)?.parse().ok()?;
    expect_char(text, 16, ":")?;
    let second: i64 = text.get(17..19)?.parse().ok()?;
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || !(0..=23).contains(&hour)
        || !(0..=59).contains(&minute)
        || !(0..=60).contains(&second)
    {
        return None;
    }

    let mut rest = text.get(19..)?;
    let mut millis = 0_i64;
    if let Some(fraction) = rest.strip_prefix('.') {
        let digits: String = fraction.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() {
            return None;
        }
        let mut first_three: String = digits.chars().take(3).collect();
        while first_three.len() < 3 {
            first_three.push('0');
        }
        millis = first_three.parse().ok()?;
        rest = fraction.get(digits.len()..)?;
    }
    let offset_seconds = match rest {
        "Z" | "z" => 0,
        _ => {
            let (sign, zone) = match rest.strip_prefix('+') {
                Some(zone) => (1, zone),
                None => (-1, rest.strip_prefix('-')?),
            };
            let zone_hour: i64 = zone.get(0..2)?.parse().ok()?;
            expect_char(zone, 2, ":")?;
            let zone_minute: i64 = zone.get(3..5)?.parse().ok()?;
            if zone.len() != 5 || zone_hour > 23 || zone_minute > 59 {
                return None;
            }
            sign * (zone_hour * 3600 + zone_minute * 60)
        }
    };
    seconds_since_epoch(year, month, day, hour, minute, second, offset_seconds)
        .map(|seconds| seconds as f64 * 1000.0 + millis as f64)
}

/// Whether `expected` (one ASCII character) is at byte `index`.
fn expect_char(text: &str, index: usize, expected: &str) -> Option<()> {
    let end = index.checked_add(expected.len())?;
    (text.get(index..end)? == expected).then_some(())
}

/// The days-from-civil algorithm (proleptic Gregorian) plus the time of day, minus the zone offset.
#[expect(
    clippy::arithmetic_side_effects,
    reason = "every input is range-checked by the caller (year 0..=9999, month 1..=12, day 1..=31, hour/minute/second small), so no i64 operation here can overflow"
)]
fn seconds_since_epoch(
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
    offset_seconds: i64,
) -> Option<i64> {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_index = (month + 9) % 12;
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    Some(days * 86_400 + hour * 3600 + minute * 60 + second - offset_seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_what_easytier_web_reports() {
        // 2026-10-29T07:23:40.084+08:00 is 2026-10-28T23:23:40.084Z.
        let ms = parse_rfc3339_ms("2026-10-29T07:23:40.084490+08:00").unwrap();
        assert_eq!(ms, 1_793_229_820_084.0);
        assert_eq!(
            parse_rfc3339_ms("2026-10-28T23:23:40.084Z"),
            Some(1_793_229_820_084.0)
        );
        assert_eq!(parse_rfc3339_ms("1970-01-01T00:00:00Z"), Some(0.0));
        assert_eq!(
            parse_rfc3339_ms("1970-01-01T00:00:01.5-01:30"),
            Some(5_401_500.0)
        );
    }

    #[test]
    fn rejects_garbage() {
        for bad in [
            "",
            "yesterday",
            "2026-13-01T00:00:00Z",
            "2026-10-29T07:23:40",
            "2026-10-29T07:23:40.+08:00",
        ] {
            assert_eq!(parse_rfc3339_ms(bad), None, "{bad}");
        }
    }
}

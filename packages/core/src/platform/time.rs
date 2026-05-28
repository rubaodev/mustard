//! `time` — the single canonical home for Unix-millis ↔ ISO-8601 conversion.
//!
//! Before this module the same two routines (Howard Hinnant's
//! `days_from_civil` / `civil_from_days`) were copy-pasted across a dozen sites
//! in `mustard-rt` — `parse_iso_millis` lived in `complete_spec`, the
//! projection fold, and two hooks; the inverse (`millis_to_iso`) in
//! `spec_clear`, `otel::diagnose`, and the NDJSON writer; and `now_millis` in
//! four hooks plus `util`. This module is the permanent single owner.
//!
//! ## Design (SOLID)
//!
//! - **Single Responsibility.** Only calendar arithmetic on Unix milliseconds.
//!   No filesystem, no events, no locale.
//! - **Pure + dependency-free.** [`parse_iso_millis`] and [`millis_to_iso`] are
//!   pure functions on their inputs (no calendar crate). Only
//!   [`now_unix_millis`] / [`now_iso8601`] read the wall clock.
//! - **One numeric convention.** Milliseconds since the Unix epoch as `i64`
//!   (good past the year 2262). Callers needing `u128` cast at the boundary.

use std::time::{SystemTime, UNIX_EPOCH};

/// Current wall-clock time as milliseconds since the Unix epoch.
///
/// Fail-safe: a clock before the epoch yields `0` rather than panicking.
#[must_use]
pub fn now_unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// `now_unix_millis` rendered as an ISO-8601 UTC string
/// (`YYYY-MM-DDTHH:MM:SS.mmmZ`) — matching JavaScript `new Date().toISOString()`.
#[must_use]
pub fn now_iso8601() -> String {
    millis_to_iso(now_unix_millis())
}

/// Render Unix milliseconds as an ISO-8601 UTC string
/// (`YYYY-MM-DDTHH:MM:SS.mmmZ`).
///
/// Uses Howard Hinnant's `civil_from_days` algorithm — no calendar crate.
/// Negative inputs (before the epoch) are clamped to `0`.
#[must_use]
pub fn millis_to_iso(ms: i64) -> String {
    let ms = ms.max(0);
    let secs = ms / 1000;
    let millis = ms % 1000;
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    format!("{year:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}.{millis:03}Z")
}

/// Parse an ISO-8601 timestamp into milliseconds since the Unix epoch.
///
/// Tolerant: only the leading `YYYY-MM-DDTHH:MM:SS` is required (any fractional
/// seconds / zone suffix is ignored). Returns `None` on malformed input.
/// Uses Howard Hinnant's `days_from_civil` — no calendar crate.
#[must_use]
pub fn parse_iso_millis(iso: &str) -> Option<i64> {
    let bytes = iso.as_bytes();
    if bytes.len() < 19 || bytes[4] != b'-' || bytes[7] != b'-' || bytes[10] != b'T' {
        return None;
    }
    let num = |s: &str| -> Option<i64> { s.parse().ok() };
    let year = num(iso.get(0..4)?)?;
    let month = num(iso.get(5..7)?)?;
    let day = num(iso.get(8..10)?)?;
    let hh = num(iso.get(11..13)?)?;
    let mm = num(iso.get(14..16)?)?;
    let ss = num(iso.get(17..19)?)?;

    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let secs = days * 86_400 + hh * 3600 + mm * 60 + ss;
    if secs < 0 {
        return Some(0);
    }
    // Optional `.sss` fraction (`YYYY-MM-DDTHH:MM:SS.mmm…`). Only the first
    // three fractional digits are read; anything past them (or a zone suffix)
    // is ignored.
    let millis = if iso.len() >= 23 && bytes.get(19) == Some(&b'.') {
        iso.get(20..23).and_then(|s| s.parse::<i64>().ok()).unwrap_or(0)
    } else {
        0
    };
    Some(secs.saturating_mul(1000) + millis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_round_trips_through_millis() {
        let iso = "2026-05-20T10:00:01.000Z";
        let ms = parse_iso_millis(iso).expect("parses");
        assert_eq!(millis_to_iso(ms), iso);
    }

    #[test]
    fn parse_rejects_malformed() {
        assert!(parse_iso_millis("nope").is_none());
        assert!(parse_iso_millis("2026/05/20").is_none());
    }

    #[test]
    fn parse_reads_fraction_and_ignores_zone_suffix() {
        let a = parse_iso_millis("2026-05-20T10:00:00Z").unwrap();
        let b = parse_iso_millis("2026-05-20T10:00:00.123+00:00").unwrap();
        // The `.123` fraction is read; the trailing zone offset is ignored.
        assert_eq!(b - a, 123);
    }

    #[test]
    fn one_second_apart() {
        let a = parse_iso_millis("2026-05-20T10:00:00Z").unwrap();
        let b = parse_iso_millis("2026-05-20T10:00:01Z").unwrap();
        assert_eq!(b - a, 1000);
    }

    #[test]
    fn now_is_after_2026() {
        // 2026-01-01T00:00:00Z in millis.
        assert!(now_unix_millis() > 1_767_225_600_000);
    }
}

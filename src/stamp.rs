//! Timestamp helpers for the drawer. bd stamps are RFC 3339 UTC, e.g.
//! `2026-11-15T09:22:25Z`. Pure apart from `now_epoch`.

use std::time::{SystemTime, UNIX_EPOCH};

/// Current Unix time in seconds.
pub fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Unix seconds for an RFC 3339 UTC stamp. Fractions and the trailing `Z`
/// are ignored; anything that is not `YYYY-MM-DDTHH:MM:SS` returns `None`.
pub fn epoch_secs(stamp: &str) -> Option<i64> {
    let b = stamp.as_bytes();
    if b.len() < 19
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> { stamp.get(r)?.parse().ok() };
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (hh, mm, ss) = (num(11..13)?, num(14..16)?, num(17..19)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || hh > 23 || mm > 59 || ss > 60 {
        return None;
    }
    Some(days_from_civil(y, m, d) * 86_400 + hh * 3_600 + mm * 60 + ss)
}

/// `2026-11-15T09:22:25Z` becomes `2026-11-15 09:22:25 UTC`. Unparseable
/// input is returned unchanged.
pub fn absolute(stamp: &str) -> String {
    if epoch_secs(stamp).is_none() {
        return stamp.to_string();
    }
    format!("{} {} UTC", &stamp[..10], &stamp[11..19])
}

/// Coarse age of `stamp` relative to `now`: `just now`, `12m ago`, `5h ago`,
/// `1d ago`. Future stamps (clock skew) read as `just now`.
pub fn ago(stamp: &str, now: i64) -> Option<String> {
    let secs = now - epoch_secs(stamp)?;
    Some(match secs {
        s if s < 60 => "just now".to_string(),
        s if s < 3_600 => format!("{}m ago", s / 60),
        s if s < 86_400 => format!("{}h ago", s / 3_600),
        s => format!("{}d ago", s / 86_400),
    })
}

/// Days since 1970-01-01 (Howard Hinnant's civil-calendar algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12; // March = 0 … February = 11
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

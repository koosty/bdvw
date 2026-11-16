//! S1: drawer timestamp formatting over fixed stamps (bdvw-rs drawer rework).

use bdvw::stamp::{absolute, ago, epoch_secs};

#[test]
fn epoch_anchor_points() {
    assert_eq!(epoch_secs("1970-01-01T00:00:00Z"), Some(0));
    // 2000-03-01 crosses the leap-day boundary in the civil-day math.
    assert_eq!(epoch_secs("2000-03-01T00:00:00Z"), Some(951_868_800));
}

#[test]
fn rejects_non_rfc3339_input() {
    assert_eq!(epoch_secs("not a date"), None);
    assert_eq!(epoch_secs("2026-13-01T00:00:00Z"), None);
    assert_eq!(epoch_secs(""), None);
}

#[test]
fn absolute_renders_utc_and_passes_garbage_through() {
    assert_eq!(absolute("2026-11-15T09:22:25Z"), "2026-11-15 09:22:25 UTC");
    assert_eq!(absolute("garbage"), "garbage");
}

#[test]
fn ago_buckets_by_largest_unit() {
    let created = epoch_secs("2026-11-15T09:22:25Z").unwrap();
    assert_eq!(
        ago("2026-11-15T09:22:25Z", created + 30),
        Some("just now".into())
    );
    assert_eq!(
        ago("2026-11-15T09:22:25Z", created + 12 * 60),
        Some("12m ago".into())
    );
    assert_eq!(
        ago("2026-11-15T09:22:25Z", created + 5 * 3_600),
        Some("5h ago".into())
    );
    assert_eq!(
        ago("2026-11-15T09:22:25Z", created + 86_400),
        Some("1d ago".into())
    );
}

#[test]
fn ago_clamps_future_stamps_to_just_now() {
    let created = epoch_secs("2026-11-15T09:22:25Z").unwrap();
    assert_eq!(
        ago("2026-11-15T09:22:25Z", created - 3_600),
        Some("just now".into())
    );
}

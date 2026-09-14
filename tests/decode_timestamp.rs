//! Timestamp rendering test (ports C++ `AuDecoderTests.JsonOutputHandler.Time`).
//!
//! Encodes a single timestamp value, decodes it back to JSON, and asserts the
//! ISO-8601 rendering with 9 fractional (nanosecond) digits — matching the C++
//! `JsonOutputHandler::onTime` output.

mod common;

use common::EncoderTestHarness;

#[test]
fn timestamp_renders_as_iso8601_with_nanos() {
    let mut h = EncoderTestHarness::new();
    h.encode(|w| {
        w.nanos(123_456_789);
    });
    assert_eq!(h.get_json(), r#""1970-01-01T00:00:00.123456789""#);
}

#[test]
fn timestamp_with_full_date_and_time() {
    // 2021-01-01T00:00:00 UTC = 1_609_459_200 s; add 987_654_321 ns.
    let nanos = 1_609_459_200 * 1_000_000_000 + 987_654_321;
    let mut h = EncoderTestHarness::new();
    h.encode(|w| {
        w.nanos(nanos);
    });
    assert_eq!(h.get_json(), r#""2021-01-01T00:00:00.987654321""#);
}

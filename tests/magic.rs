//! au header/magic detection tests (ported from C++ `AuMagicTest.cpp`).
//!
//! These cover the pure detection functions `looks_like_au_header` and
//! `is_au_file`. The related "unsupported version is reported as a version
//! error, not as non-au input" behavior lives in `tests/header.rs`, which
//! exercises it end-to-end through `parse_stream`.

use au::byte_source::BufferByteSource;
use au::magic::{AU_MAGIC_PREFIX_LEN, is_au_file, looks_like_au_header};

/// magic, smallint version 1, empty metadata string, record terminator.
const V1_HEADER: &[u8] = b"HAU\x61\x20\x0f\n";

// ---- looks_like_au_header ----

#[test]
fn accepts_supported_version() {
    assert!(looks_like_au_header(b"HAU\x61"));
}

#[test]
fn accepts_versions_we_do_not_support() {
    assert!(looks_like_au_header(b"HAU\x62")); // version 2
    assert!(looks_like_au_header(b"HAU\x60")); // version 0
    assert!(looks_like_au_header(b"HAU\x7f")); // version 31, largest smallint
    assert!(looks_like_au_header(b"HAU\x06")); // varint-encoded version
}

#[test]
fn ignores_trailing_bytes() {
    assert!(looks_like_au_header(V1_HEADER));
    assert!(looks_like_au_header(b"HAU\x61 and then some other stuff"));
}

#[test]
fn rejects_short_input() {
    assert!(!looks_like_au_header(b""));
    assert!(!looks_like_au_header(b"H"));
    assert!(!looks_like_au_header(b"HA"));
    assert!(!looks_like_au_header(b"HAU")); // magic but no version byte
}

#[test]
fn rejects_wrong_magic() {
    assert!(!looks_like_au_header(b"hau\x61"));
    assert!(!looks_like_au_header(b"XAU\x61"));
    assert!(!looks_like_au_header(b"HAX\x61"));
    assert!(!looks_like_au_header(b"{\"a\":1}"));
}

#[test]
fn rejects_implausible_version_byte() {
    assert!(!looks_like_au_header(b"HAU\x00"));
    assert!(!looks_like_au_header(b"HAUZ")); // 0x5a, a small *negative* int
    assert!(!looks_like_au_header(b"HAU\x05")); // string marker, not a version
    assert!(!looks_like_au_header(b"HAU\x80")); // dict ref, high bit set
    assert!(!looks_like_au_header(b"HAU\xff"));
}

// ---- is_au_file (non-consuming source detection) ----

#[test]
fn detects_header_and_leaves_position() {
    let source = BufferByteSource::new(V1_HEADER);
    assert!(is_au_file(&source));
    assert_eq!(0, source.pos(), "is_au_file must not consume the header");
    // Repeatable.
    assert!(is_au_file(&source));
    assert_eq!(0, source.pos());
}

#[test]
fn detects_unsupported_version() {
    let mut v2 = V1_HEADER.to_vec();
    v2[3] = 0x62; // version 2
    let source = BufferByteSource::new(&v2);
    assert!(is_au_file(&source));
}

#[test]
fn rejects_json() {
    let json = b"{\"a\":1,\"b\":2.5}\n";
    let source = BufferByteSource::new(json);
    assert!(!is_au_file(&source));
    assert_eq!(0, source.pos());
}

#[test]
fn rejects_source_too_short_to_hold_a_header() {
    for len in 1..AU_MAGIC_PREFIX_LEN {
        let source = BufferByteSource::new(&V1_HEADER[0..len]);
        assert!(!is_au_file(&source), "for length {len}");
    }
}

#[test]
fn rejects_empty_source() {
    let source = BufferByteSource::new(b"");
    assert!(!is_au_file(&source));
}

//! Malformed-input tests: crafted/hostile byte streams must surface a
//! `ParseError`, never panic (arithmetic under/overflow, out-of-range slicing).
//!
//! These guard the decode path against attacker-controlled lengths and
//! backrefs. In debug builds `overflow-checks` are on, so a regression here
//! turns straight into a panic and fails the test.

mod common;

use au::byte_source::BufferByteSource;
use au::decoder::parse_stream;
use au::dictionary::Dictionary;
use au::error::ParseError;
use au::record_handler::AuRecordHandler;
use common::StringCollectingJsonHandler;

/// Run the decoder over raw bytes without requiring a header, so a single
/// crafted record can be fed in as the first record (its start-of-record is 0).
fn parse(bytes: &[u8]) -> Result<(), ParseError> {
    let mut source = BufferByteSource::new(bytes);
    let mut dictionary = Dictionary::new();
    let mut handler = StringCollectingJsonHandler::new();
    let mut record_handler = AuRecordHandler::new(&mut dictionary, &mut handler);
    parse_stream(&mut source, &mut record_handler, false)
}

// ---- byte_source: length additions must not overflow usize ----

#[test]
fn read_bytes_huge_len_errors_without_overflow() {
    let mut source = BufferByteSource::new(b"abc");
    // Advance so pos > 0: `pos + usize::MAX` would overflow the addition.
    assert_eq!(source.next_byte(), Some(b'a'));
    assert!(source.read_bytes(usize::MAX).is_err());
    // Position is unchanged by the failed read, so subsequent reads still work.
    assert_eq!(source.read_bytes(2).unwrap(), b"bc");
}

#[test]
fn skip_huge_len_errors_without_overflow() {
    let mut source = BufferByteSource::new(b"abc");
    assert_eq!(source.next_byte(), Some(b'a'));
    assert!(source.skip(usize::MAX).is_err());
    assert_eq!(source.pos(), 1);
}

// ---- dictionary: backref larger than the position must not underflow ----

#[test]
fn find_dictionary_ref_backref_before_start_errors() {
    let dict = Dictionary::new();
    // sor - rel_dict_pos would underflow (0 - 1); expect an error, not a panic.
    assert!(dict.find_dictionary_ref(0, 1).is_err());
    assert!(dict.find_dictionary_ref(10, u32::MAX as usize).is_err());
}

#[test]
fn find_dictionary_backref_before_start_errors() {
    let mut dict = Dictionary::new();
    assert!(dict.find_dictionary(0, 1).is_err());
    assert!(dict.find_dictionary(10, u32::MAX as usize).is_err());
}

// ---- decoder: 'V' record length shorter than its 2-byte terminator ----

#[test]
fn value_record_len_zero_errors() {
    // 'V', 4-byte backref = 0, varint len = 0. `len - 2` would underflow.
    let bytes = [b'V', 0, 0, 0, 0, 0x00];
    assert!(parse(&bytes).is_err());
}

#[test]
fn value_record_len_one_errors() {
    // 'V', 4-byte backref = 0, varint len = 1. `len - 2` would underflow.
    let bytes = [b'V', 0, 0, 0, 0, 0x01];
    assert!(parse(&bytes).is_err());
}

/// A well-formed 'V' record whose backref points before the start of the
/// stream must not panic: the backref simply fails to resolve and the value
/// bytes are skipped. (len = 2 -> zero value bytes, then the terminator.)
#[test]
fn value_record_backref_before_start_does_not_panic() {
    let bytes = [
        b'V',
        0xFF,
        0xFF,
        0xFF,
        0xFF, // backref far beyond position 0
        0x02, // varint len = 2 (value + terminator, zero value bytes)
        0x0f, // Marker::RecordEnd
        b'\n',
    ];
    // Either Ok or Err is acceptable; the point is that it returns rather than panicking.
    let _ = parse(&bytes);
}

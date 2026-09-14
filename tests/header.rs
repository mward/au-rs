//! Header / format-version detection tests.
//!
//! Ports the version-handling subset of the C++ `AuMagicTest.cpp`: a valid
//! `HAU`+version header parses; an unsupported version is reported as a version
//! error (distinct from "not an au file"); wrong magic / truncated input is
//! rejected. The non-consuming `is_au_file` sniff from `AuMagic.h` is covered
//! separately once that feature lands (see `tests/magic.rs`).

mod common;

use au::byte_source::BufferByteSource;
use au::common::SMALL_INT_POSITIVE;
use au::decoder::parse_stream;
use au::dictionary::Dictionary;
use au::encoder::AuEncoder;
use au::error::ParseError;
use au::record_handler::AuRecordHandler;
use common::StringCollectingJsonHandler;

/// Encode a one-record stream and return its bytes (starts with the `HAU`
/// header the encoder emits).
fn encode_one_record() -> Vec<u8> {
    let mut enc = AuEncoder::new();
    let mut out = Vec::new();
    enc.encode(
        |w| {
            w.value_i64(42);
        },
        |a, b| {
            out.extend_from_slice(a);
            out.extend_from_slice(b);
            a.len() + b.len()
        },
    );
    out
}

fn parse(bytes: &[u8]) -> Result<(), ParseError> {
    let mut source = BufferByteSource::new(bytes);
    let mut dictionary = Dictionary::new();
    let mut handler = StringCollectingJsonHandler::new();
    let mut record_handler = AuRecordHandler::new(&mut dictionary, &mut handler);
    parse_stream(&mut source, &mut record_handler, true)
}

#[test]
fn accepts_supported_version() {
    let bytes = encode_one_record();
    // Sanity-check the header layout the rest of the test mutates: 'H','A','U'
    // then the version encoded as a positive small-int (version 1 -> 0x61).
    assert_eq!(&bytes[0..3], b"HAU");
    assert_eq!(bytes[3], SMALL_INT_POSITIVE | 1);
    assert!(parse(&bytes).is_ok());
}

#[test]
fn rejects_unsupported_version_as_version_error() {
    let mut bytes = encode_one_record();
    // Bump the version small-int to 2, which this build does not support.
    bytes[3] = SMALL_INT_POSITIVE | 2;
    let err = parse(&bytes).expect_err("unsupported version must error");
    assert!(
        err.to_string().contains("format version"),
        "expected a format-version error, got: {err}"
    );
}

#[test]
fn rejects_wrong_magic() {
    let mut bytes = encode_one_record();
    bytes[1] = b'X'; // 'HAU' -> 'HXU'
    assert!(parse(&bytes).is_err());
}

#[test]
fn rejects_input_not_starting_with_header() {
    // Doesn't begin with 'H': must be rejected as not an au header.
    let err = parse(b"{\"json\":true}\n").expect_err("json is not au");
    assert!(err.to_string().contains("au header"), "got: {err}");
}

#[test]
fn rejects_short_input() {
    let bytes = encode_one_record();
    // Truncate to just "HA" — too short to hold a header record.
    assert!(parse(&bytes[0..2]).is_err());
}

#[test]
fn empty_input_is_ok() {
    // No records at all: parse_stream over empty input succeeds (nothing to do).
    assert!(parse(b"").is_ok());
}

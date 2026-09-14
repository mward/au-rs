//! Coverage-guided fuzz target for the au decoder.
//!
//! Feeds arbitrary bytes through `parse_stream` and requires that it always
//! returns (Ok or Err) rather than panicking, overflowing, or hanging. This is
//! the coverage-guided complement to the random-input `tests/proptest.rs`
//! never-panic properties and the fixed-corpus `tests/fixtures.rs` test.
//!
//! Run (needs nightly + cargo-fuzz):
//!   cargo +nightly fuzz run parse_stream
//! The seed corpus in `fuzz/corpus/parse_stream/` is the same set of AFL seeds
//! and hand-corrupted samples used by `tests/fixtures.rs`.

#![no_main]

use au::byte_source::BufferByteSource;
use au::decoder::parse_stream;
use au::dictionary::Dictionary;
use au::handler::ValueHandler;
use au::record_handler::AuRecordHandler;
use libfuzzer_sys::fuzz_target;

/// Discards every decoded value; we only care that decoding terminates.
#[derive(Default)]
struct Discard;
impl ValueHandler for Discard {}

fn run(data: &[u8], with_header: bool) {
    let mut source = BufferByteSource::new(data);
    let mut dictionary = Dictionary::new();
    let mut handler = Discard;
    let mut record_handler = AuRecordHandler::new(&mut dictionary, &mut handler);
    let _ = parse_stream(&mut source, &mut record_handler, with_header);
}

fuzz_target!(|data: &[u8]| {
    // Exercise both entry points: header-required (whole-file) and
    // header-optional (a single crafted record).
    run(data, true);
    run(data, false);
});

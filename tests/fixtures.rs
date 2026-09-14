//! Fixture-driven decoder robustness test.
//!
//! Ports the C++ `AuDecoderTestCases.doesntCrashOnCases` harness: decode a
//! corpus of real files and require the decoder to *return* on each — an `Err`
//! is fine, a panic or stack overflow is not.
//!
//! The corpus (`tests/fixtures/corpus/`) is copied verbatim from upstream:
//!   * `afl-1.au`, `afl-2.au` — AFL fuzzer seeds in the current `HAU` format
//!     (from the C++ `test/cases/` directory this test mirrors).
//!   * `TailDictOrder*.au`, `Tail-CorruptLength.au`, `time.au` — legacy
//!     `tail`-feature samples using an older header (`HI\x01`, not `HAU`).
//!     The current codec rejects them at the header; they're kept here as
//!     additional "unexpected input must not crash" cases.
//!
//! Note on stack: `afl-2.au` nests to the decoder's `MAX_DEPTH` (2048, matching
//! C++), which returns "File too deeply nested" — but 2048 recursion frames
//! exceed the ~2 MiB stack Rust gives test threads. The C++ gtest runs on the
//! main thread (8 MiB), so to reproduce the same conditions we run the decode
//! on a thread with a large stack.

mod common;

use au::byte_source::BufferByteSource;
use au::decoder::parse_stream;
use au::dictionary::Dictionary;
use au::record_handler::AuRecordHandler;
use common::StringCollectingJsonHandler;
use std::fs;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Decode every `.au` fixture in the corpus and assert the decoder returns
/// (Ok or Err) without crashing. In debug builds `overflow-checks` are on, so
/// any arithmetic under/overflow would panic and fail this test.
#[test]
fn decoder_does_not_crash_on_corpus() {
    // Run on a generous stack so the depth-limited (but deep) recursion in
    // `afl-2.au` reaches its "too deeply nested" error instead of overflowing
    // the small default test-thread stack. See the module comment.
    let handle = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(run_corpus)
        .unwrap();
    handle.join().expect("corpus decode thread must not crash");
}

fn run_corpus() {
    let corpus = fixtures_dir().join("corpus");
    let mut checked = 0usize;

    for entry in fs::read_dir(&corpus).expect("corpus dir should exist") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("au") {
            continue;
        }
        let bytes = fs::read(&path).unwrap();

        let mut source = BufferByteSource::new(&bytes);
        let mut dictionary = Dictionary::new();
        let mut handler = StringCollectingJsonHandler::new();
        let mut record_handler = AuRecordHandler::new(&mut dictionary, &mut handler);
        // Return value intentionally ignored: malformed / legacy-format input
        // surfaces as an Err, and the only failure this test guards against is
        // a panic (or stack overflow, caught by the join above).
        let _ = parse_stream(&mut source, &mut record_handler, true);

        checked += 1;
    }

    assert!(
        checked > 0,
        "expected at least one .au fixture in {corpus:?}"
    );
}

# au

A Rust implementation of the **au** format — a compact, self-describing binary
serialization for streams of structured records. Think line-delimited JSON, but
encoded as a binary stream with per-stream **string interning** so that keys and
repeated string values are written once and referenced by a small back-reference
thereafter.

This is a port of the original C++ `au` library.

## Why au?

Log and event streams are enormously repetitive: the same object keys and the
same string values recur on nearly every record. au exploits this by maintaining
a running **dictionary** of interned strings. The first time a string is seen it
is added to the dictionary; subsequent occurrences are encoded as a compact
integer reference. The result is a stream that is:

- **Compact** — repeated keys/values cost a varint, not a full string.
- **Streaming** — records are encoded and decoded one at a time; the dictionary
  is built incrementally and is periodically re-indexed and purged so it stays
  bounded on long-running streams.
- **Self-describing** — a header record carries a format version and optional
  metadata, and every value is tagged with its type.

## Format at a glance

A stream is a sequence of records, each introduced by a tag byte:

| Tag | Record        | Purpose                                             |
|-----|---------------|-----------------------------------------------------|
| `H` | Header        | Magic (`HAU`), format version, metadata string      |
| `C` | Dict clear    | Reset the interning dictionary                       |
| `A` | Dict add      | Append newly interned strings to the dictionary      |
| `V` | Value         | An encoded value (scalar, array, or object)          |

Values are tagged by a one-byte [`Marker`](src/common.rs) (null, bool, double,
timestamp, string, varint, int64, dict-ref, array/object start & end, …). Small
integers in `-32..32` are packed into a single byte via the `SMALL_INT_POSITIVE`
/ `SMALL_INT_NEGATIVE` tags.

## Usage

### Encoding

`AuEncoder::encode` takes a closure that writes one record using an
[`AuWriter`](src/writer.rs), plus a sink closure that receives the two byte
slices to emit (dictionary additions followed by the record body):

```rust
use au::encoder::AuEncoder;

let mut encoder = AuEncoder::new();
let mut out: Vec<u8> = Vec::new();

encoder.encode(
    |w| {
        w.map(|w| {
            w.kv_str("event", "login");
            w.kv_str("user", "alice");
            w.kv_i64("attempt", 1);
            w.kv_bool("ok", true);
        });
    },
    |dict_bytes, record_bytes| {
        out.extend_from_slice(dict_bytes);
        out.extend_from_slice(record_bytes);
        dict_bytes.len() + record_bytes.len()
    },
);
```

The `AuWriter` API is fluent and supports scalars (`value_i64`, `value_f64`,
`value_str`, `value_bool`, `null`, `nanos`), containers (`map`, `array`,
`start_map`/`end_map`, `start_array`/`end_array`), and key/value helpers
(`kv_str`, `kv_i64`, `kv_u64`, `kv_f64`, `kv_bool`).

### Decoding

Decoding is push-based: you implement a [`ValueHandler`](src/handler.rs), wrap
it in an [`AuRecordHandler`](src/record_handler.rs) (which resolves dictionary
references), and drive the parser with `parse_stream`. Your handler's callbacks
fire as each value is decoded — no intermediate representation is built unless
you choose to build one:

```rust
use au::byte_source::BufferByteSource;
use au::decoder::parse_stream;
use au::dictionary::Dictionary;
use au::handler::ValueHandler;
use au::record_handler::AuRecordHandler;

// Every `ValueHandler` method has a default no-op impl, so override only the
// callbacks you care about.
#[derive(Default)]
struct MyHandler;
impl ValueHandler for MyHandler {
    fn on_int(&mut self, _pos: usize, v: i64) {
        println!("int: {v}");
    }
    // ...override on_object_start, on_string_end, etc. as needed
}

let mut source = BufferByteSource::new(&out);
let mut dictionary = Dictionary::new();
let mut value_handler = MyHandler::default();
let mut handler = AuRecordHandler::new(&mut dictionary, &mut value_handler);

parse_stream(&mut source, &mut handler, /* expect_header = */ true).unwrap();
```

The crate's integration tests include a `ValueHandler` that reconstructs each
record as a `serde_json::Value` (see `tests/common/mod.rs`); it lives in the test
tree only, so the library itself carries **no runtime dependencies**.

## Dictionary tuning

`AuEncoder::with_options` / `with_full_options` control how the interning
dictionary is maintained over a long stream:

- **`purge_interval` / `purge_threshold`** — how often, and below what usage
  count, to drop rarely-used interned strings.
- **`reindex_interval`** — how often to re-index the dictionary so frequently
  used strings get the shortest references.

You can also drive these manually via `purge_dictionary`, `reindex_dictionary`,
and `clear_dictionary`.

## Module layout

| Module              | Responsibility                                        |
|---------------------|-------------------------------------------------------|
| `encoder`           | `AuEncoder` — record framing, dictionary lifecycle    |
| `writer`            | `AuWriter` — fluent value/record encoding             |
| `decoder`           | `parse_value` / `parse_record` / `parse_stream`       |
| `handler`           | `ValueHandler` / `RecordHandler` traits               |
| `record_handler`    | `AuRecordHandler` bridging records to a value handler  |
| `string_intern`     | Interning table used while encoding                   |
| `dictionary`        | Dictionary state used while decoding                  |
| `buffer`            | `VectorBuffer` output buffer                           |
| `byte_source`       | `BufferByteSource` input cursor                        |
| `common`            | Format constants and the `Marker` enum                 |
| `error`             | `ParseError`                                           |

## Building & testing

```sh
cargo build
cargo test
cargo clippy --all-targets
```

Requires a Rust toolchain with **edition 2024** support.

## Fuzzing

A coverage-guided [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz) /
libFuzzer target for the decoder lives in `fuzz/`. It feeds arbitrary bytes
through `parse_stream` and asserts the decoder always returns (Ok or Err) rather
than panicking, overflowing, or hanging — complementing the random-input
property tests (`tests/proptest.rs`) and the fixed-corpus test
(`tests/fixtures.rs`).

One-time setup (requires a nightly toolchain):

```sh
cargo install cargo-fuzz
```

Run the target (seeded from `fuzz/corpus/parse_stream/`):

```sh
cargo +nightly fuzz run parse_stream
# time-boxed, e.g. one minute:
cargo +nightly fuzz run parse_stream -- -max_total_time=60
```

If a crash is found, the offending input is written to
`fuzz/artifacts/parse_stream/`; replay it with:

```sh
cargo +nightly fuzz run parse_stream fuzz/artifacts/parse_stream/<crash-file>
```

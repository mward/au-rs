//! Property-based round-trip tests.
//!
//! The core invariant of the format is that anything written with `AuWriter`
//! (through `AuEncoder`) decodes back to the same logical value. We generate
//! arbitrary values, encode them, decode them back into `serde_json::Value`s,
//! and assert the result matches what the value should produce.
//!
//! Note: we compare decoded `serde_json::Value`s directly rather than routing
//! through a JSON string. serde_json's decimal float formatting is not always
//! bit-exact, whereas au's binary f64 encoding is, so a text round trip would
//! test serde_json rather than au.

mod common;

use au::byte_source::BufferByteSource;
use au::decoder::parse_stream;
use au::dictionary::Dictionary;
use au::encoder::AuEncoder;
use au::handler::ValueHandler;
use au::record_handler::AuRecordHandler;
use au::writer::AuWriter;
use common::StringCollectingJsonHandler;
use proptest::prelude::*;
use serde_json::Value as JsonValue;

/// A value that mirrors exactly what the au format can represent, so we can
/// both drive the writer and predict the decoded value without ambiguity.
#[derive(Debug, Clone)]
enum AuValue {
    Null,
    Bool(bool),
    I64(i64),
    U64(u64),
    F64(f64), // finite only; NaN/Inf are decoded to strings, tested separately
    Str(String),
    Array(Vec<AuValue>),
    /// Keys are unique (generated from a map), so order/dedup never matter.
    Object(Vec<(String, AuValue)>),
}

fn write_value(w: &mut AuWriter, v: &AuValue) {
    match v {
        AuValue::Null => {
            w.null();
        }
        AuValue::Bool(b) => {
            w.value_bool(*b);
        }
        AuValue::I64(i) => {
            w.value_i64(*i);
        }
        AuValue::U64(u) => {
            w.value_u64(*u);
        }
        AuValue::F64(f) => {
            w.value_f64(*f);
        }
        AuValue::Str(s) => {
            w.value_str(s);
        }
        AuValue::Array(items) => {
            w.array(|w| {
                for it in items {
                    write_value(w, it);
                }
            });
        }
        AuValue::Object(pairs) => {
            w.map(|w| {
                for (k, val) in pairs {
                    w.key(k);
                    write_value(w, val);
                }
            });
        }
    }
}

/// The value the decoder is expected to reconstruct.
fn expected(v: &AuValue) -> JsonValue {
    match v {
        AuValue::Null => JsonValue::Null,
        AuValue::Bool(b) => JsonValue::Bool(*b),
        AuValue::I64(i) => JsonValue::Number((*i).into()),
        AuValue::U64(u) => JsonValue::Number((*u).into()),
        AuValue::F64(f) => JsonValue::Number(
            serde_json::Number::from_f64(*f).expect("finite float is representable"),
        ),
        AuValue::Str(s) => JsonValue::String(s.clone()),
        AuValue::Array(items) => JsonValue::Array(items.iter().map(expected).collect()),
        AuValue::Object(pairs) => {
            let mut map = serde_json::Map::new();
            for (k, val) in pairs {
                map.insert(k.clone(), expected(val));
            }
            JsonValue::Object(map)
        }
    }
}

// ---- encode / decode helpers (au API only, no JSON text round trip) ----

fn encode_records(values: &[AuValue]) -> Vec<u8> {
    let mut enc = AuEncoder::new();
    let mut out = Vec::new();
    for v in values {
        let sink = &mut out;
        enc.encode(
            |w| write_value(w, v),
            |a, b| {
                sink.extend_from_slice(a);
                sink.extend_from_slice(b);
                a.len() + b.len()
            },
        );
    }
    out
}

/// Decode a stream that is expected to contain exactly one record.
fn decode_one(bytes: &[u8]) -> Option<JsonValue> {
    let mut source = BufferByteSource::new(bytes);
    let mut dictionary = Dictionary::new();
    let mut handler = StringCollectingJsonHandler::new();
    let mut record_handler = AuRecordHandler::new(&mut dictionary, &mut handler);
    parse_stream(&mut source, &mut record_handler, true).unwrap();
    drop(record_handler);
    handler.take_result()
}

/// Collects one decoded `serde_json::Value` per record.
///
/// A record's completed value is grabbed when the *next* record's top-level
/// container/scalar starts (and the last one at the end). Records here are
/// always objects, so `on_object_start` reliably marks the boundary.
struct RecordCollector {
    inner: StringCollectingJsonHandler,
    results: Vec<JsonValue>,
}

impl RecordCollector {
    const fn new() -> Self {
        RecordCollector {
            inner: StringCollectingJsonHandler::new(),
            results: Vec::new(),
        }
    }

    fn collect(&mut self) {
        if let Some(v) = self.inner.take_result() {
            self.results.push(v);
        }
    }
}

impl ValueHandler for RecordCollector {
    fn on_object_start(&mut self) {
        self.collect();
        self.inner.on_object_start();
    }
    fn on_object_end(&mut self) {
        self.inner.on_object_end();
    }
    fn on_array_start(&mut self) {
        self.collect();
        self.inner.on_array_start();
    }
    fn on_array_end(&mut self) {
        self.inner.on_array_end();
    }
    fn on_null(&mut self, pos: usize) {
        self.collect();
        self.inner.on_null(pos);
    }
    fn on_bool(&mut self, pos: usize, val: bool) {
        self.collect();
        self.inner.on_bool(pos, val);
    }
    fn on_int(&mut self, pos: usize, val: i64) {
        self.collect();
        self.inner.on_int(pos, val);
    }
    fn on_uint(&mut self, pos: usize, val: u64) {
        self.collect();
        self.inner.on_uint(pos, val);
    }
    fn on_double(&mut self, pos: usize, val: f64) {
        self.collect();
        self.inner.on_double(pos, val);
    }
    fn on_time(&mut self, pos: usize, nanos: u64) {
        self.collect();
        self.inner.on_time(pos, nanos);
    }
    fn on_dict_ref(&mut self, pos: usize, dict_idx: usize) {
        self.inner.on_dict_ref(pos, dict_idx);
    }
    fn on_string_start(&mut self, sov: usize, length: usize) {
        self.inner.on_string_start(sov, length);
    }
    fn on_string_end(&mut self) {
        self.inner.on_string_end();
    }
    fn on_string_fragment(&mut self, fragment: &[u8]) {
        self.inner.on_string_fragment(fragment);
    }
}

fn decode_all(bytes: &[u8]) -> Vec<JsonValue> {
    let mut source = BufferByteSource::new(bytes);
    let mut dictionary = Dictionary::new();
    let mut collector = RecordCollector::new();
    let mut record_handler = AuRecordHandler::new(&mut dictionary, &mut collector);
    parse_stream(&mut source, &mut record_handler, true).unwrap();
    drop(record_handler);
    collector.collect(); // flush the final record
    collector.results
}

fn check_roundtrip(v: &AuValue) -> Result<(), TestCaseError> {
    let bytes = encode_records(std::slice::from_ref(v));
    let got = decode_one(&bytes).ok_or_else(|| TestCaseError::fail("no record decoded"))?;
    prop_assert_eq!(got, expected(v));
    Ok(())
}

// ---- strategies ----

fn arb_scalar() -> impl Strategy<Value = AuValue> {
    prop_oneof![
        Just(AuValue::Null),
        any::<bool>().prop_map(AuValue::Bool),
        any::<i64>().prop_map(AuValue::I64),
        any::<u64>().prop_map(AuValue::U64),
        any::<f64>()
            .prop_filter("finite only", |f| f.is_finite())
            .prop_map(AuValue::F64),
        any::<String>().prop_map(AuValue::Str),
    ]
}

/// Object keys: mix of tiny (not interned) and longer (interned) keys.
fn arb_key() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z0-9_ ]{0,16}").unwrap()
}

fn arb_au_value() -> impl Strategy<Value = AuValue> {
    arb_scalar().prop_recursive(4, 48, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(AuValue::Array),
            prop::collection::hash_map(arb_key(), inner, 0..6)
                .prop_map(|m| AuValue::Object(m.into_iter().collect())),
        ]
    })
}

/// A top-level object (the primary use case, and safe for multi-record framing).
fn arb_object() -> impl Strategy<Value = AuValue> {
    prop::collection::hash_map(arb_key(), arb_au_value(), 0..6)
        .prop_map(|m| AuValue::Object(m.into_iter().collect()))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Any single value survives an encode -> decode round trip.
    #[test]
    fn roundtrip_single_record(v in arb_au_value()) {
        check_roundtrip(&v)?;
    }

    #[test]
    fn roundtrip_i64(i in any::<i64>()) {
        check_roundtrip(&AuValue::I64(i))?;
    }

    #[test]
    fn roundtrip_u64(u in any::<u64>()) {
        check_roundtrip(&AuValue::U64(u))?;
    }

    #[test]
    fn roundtrip_f64(f in any::<f64>().prop_filter("finite only", |f| f.is_finite())) {
        check_roundtrip(&AuValue::F64(f))?;
    }

    /// Arbitrary Unicode strings, covering inline (<=31 bytes) and length-prefixed forms.
    #[test]
    fn roundtrip_string(s in any::<String>()) {
        check_roundtrip(&AuValue::Str(s))?;
    }

    /// Multiple records in one stream each decode back independently.
    #[test]
    fn roundtrip_multi_record(records in prop::collection::vec(arb_object(), 1..8)) {
        let bytes = encode_records(&records);
        let decoded = decode_all(&bytes);
        prop_assert_eq!(decoded.len(), records.len());
        for (got, r) in decoded.iter().zip(&records) {
            prop_assert_eq!(got, &expected(r));
        }
    }

    /// A repeated key/value pair exercises the interning + dictionary backref path
    /// across many records (value is interned once the frequency threshold is hit).
    #[test]
    fn roundtrip_repeated_interning(
        key in "[a-zA-Z]{5,20}",
        val in "[a-zA-Z]{5,20}",
        n in 1usize..40,
    ) {
        let record = AuValue::Object(vec![(key, AuValue::Str(val))]);
        let records: Vec<AuValue> = std::iter::repeat_n(record, n).collect();
        let bytes = encode_records(&records);
        let decoded = decode_all(&bytes);
        prop_assert_eq!(decoded.len(), n);
        for got in &decoded {
            prop_assert_eq!(got, &expected(&records[0]));
        }
    }
}

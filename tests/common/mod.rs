//! Shared helpers for the integration tests.
//!
//! Contains a `ValueHandler` that reconstructs each decoded record as a
//! `serde_json::Value` (previously `src/json_handler.rs`), plus a small
//! encode/decode round-trip harness.

#![allow(dead_code)]

use au::byte_source::BufferByteSource;
use au::decoder::parse_stream;
use au::dictionary::Dictionary;
use au::encoder::AuEncoder;
use au::handler::ValueHandler;
use au::writer::AuWriter;
use serde_json::Value as JsonValue;

// ============================================================
// JSON-reconstructing value handler
// ============================================================

/// A value handler that builds a serde_json::Value for each record.
pub struct JsonOutputHandler {
    stack: Vec<BuildState>,
    result: Option<JsonValue>,
}

enum BuildState {
    /// Collecting values for an array
    Array(Vec<JsonValue>),
    /// Collecting key-value pairs for an object; pending_key is the last key seen
    Object {
        map: serde_json::Map<String, JsonValue>,
        pending_key: Option<String>,
    },
}

impl JsonOutputHandler {
    pub const fn new() -> Self {
        JsonOutputHandler {
            stack: Vec::new(),
            result: None,
        }
    }

    pub const fn take_result(&mut self) -> Option<JsonValue> {
        self.result.take()
    }

    fn push_value(&mut self, val: JsonValue) {
        match self.stack.last_mut() {
            Some(BuildState::Array(arr)) => {
                arr.push(val);
            }
            Some(BuildState::Object { map, pending_key }) => {
                if let Some(key) = pending_key.take() {
                    map.insert(key, val);
                } else {
                    // Value is being used as a key (string)
                    if let JsonValue::String(s) = val {
                        *pending_key = Some(s);
                    }
                }
            }
            None => {
                self.result = Some(val);
            }
        }
    }

    fn format_timestamp(nanos: u64) -> String {
        let secs = (nanos / 1_000_000_000) as i64;
        let frac = nanos % 1_000_000_000;

        // Simple UTC time formatting
        let days_since_epoch = secs / 86400;
        let time_of_day = secs % 86400;
        let (hour, min, sec) = (
            time_of_day / 3600,
            (time_of_day % 3600) / 60,
            time_of_day % 60,
        );

        // Compute date from days since epoch (civil_from_days algorithm)
        let z = days_since_epoch + 719468;
        let era = if z >= 0 { z } else { z - 146096 } / 146097;
        let doe = (z - era * 146097) as u64;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y = yoe as i64 + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = if m <= 2 { y + 1 } else { y };

        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}",
            y, m, d, hour, min, sec, frac
        )
    }
}

impl Default for JsonOutputHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ValueHandler for JsonOutputHandler {
    fn on_object_start(&mut self) {
        self.stack.push(BuildState::Object {
            map: serde_json::Map::new(),
            pending_key: None,
        });
    }

    fn on_object_end(&mut self) {
        if let Some(BuildState::Object { map, .. }) = self.stack.pop() {
            self.push_value(JsonValue::Object(map));
        }
    }

    fn on_array_start(&mut self) {
        self.stack.push(BuildState::Array(Vec::new()));
    }

    fn on_array_end(&mut self) {
        if let Some(BuildState::Array(arr)) = self.stack.pop() {
            self.push_value(JsonValue::Array(arr));
        }
    }

    fn on_null(&mut self, _pos: usize) {
        self.push_value(JsonValue::Null);
    }

    fn on_bool(&mut self, _pos: usize, val: bool) {
        self.push_value(JsonValue::Bool(val));
    }

    fn on_int(&mut self, _pos: usize, val: i64) {
        self.push_value(JsonValue::Number(serde_json::Number::from(val)));
    }

    fn on_uint(&mut self, _pos: usize, val: u64) {
        self.push_value(JsonValue::Number(serde_json::Number::from(val)));
    }

    fn on_double(&mut self, _pos: usize, val: f64) {
        if val.is_finite() {
            if let Some(n) = serde_json::Number::from_f64(val) {
                self.push_value(JsonValue::Number(n));
            } else {
                self.push_value(JsonValue::Null);
            }
        } else if val.is_nan() {
            // JSON doesn't support NaN, represent as string
            self.push_value(JsonValue::String("NaN".to_string()));
        } else if val.is_sign_negative() {
            self.push_value(JsonValue::String("-Infinity".to_string()));
        } else {
            self.push_value(JsonValue::String("Infinity".to_string()));
        }
    }

    fn on_time(&mut self, _pos: usize, nanos: u64) {
        self.push_value(JsonValue::String(Self::format_timestamp(nanos)));
    }

    fn on_dict_ref(&mut self, _pos: usize, _dict_idx: usize) {
        // Dict refs are resolved by the DictValueHandler wrapper before reaching here
        // If we get here directly, it means no dictionary context is available
        self.push_value(JsonValue::String(format!("<dictref:{}>", _dict_idx)));
    }

    fn on_string_start(&mut self, _sov: usize, _length: usize) {}

    fn on_string_end(&mut self) {}

    fn on_string_fragment(&mut self, _fragment: &[u8]) {}
}

/// A wrapper around JsonOutputHandler that collects string fragments
pub struct StringCollectingJsonHandler {
    inner: JsonOutputHandler,
    str_buf: Vec<u8>,
}

impl StringCollectingJsonHandler {
    pub const fn new() -> Self {
        StringCollectingJsonHandler {
            inner: JsonOutputHandler::new(),
            str_buf: Vec::new(),
        }
    }

    pub const fn take_result(&mut self) -> Option<JsonValue> {
        self.inner.take_result()
    }
}

impl Default for StringCollectingJsonHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ValueHandler for StringCollectingJsonHandler {
    fn on_object_start(&mut self) {
        self.inner.on_object_start();
    }
    fn on_object_end(&mut self) {
        self.inner.on_object_end();
    }
    fn on_array_start(&mut self) {
        self.inner.on_array_start();
    }
    fn on_array_end(&mut self) {
        self.inner.on_array_end();
    }
    fn on_null(&mut self, pos: usize) {
        self.inner.on_null(pos);
    }
    fn on_bool(&mut self, pos: usize, val: bool) {
        self.inner.on_bool(pos, val);
    }
    fn on_int(&mut self, pos: usize, val: i64) {
        self.inner.on_int(pos, val);
    }
    fn on_uint(&mut self, pos: usize, val: u64) {
        self.inner.on_uint(pos, val);
    }
    fn on_double(&mut self, pos: usize, val: f64) {
        self.inner.on_double(pos, val);
    }
    fn on_time(&mut self, pos: usize, nanos: u64) {
        self.inner.on_time(pos, nanos);
    }
    fn on_dict_ref(&mut self, pos: usize, dict_idx: usize) {
        self.inner.on_dict_ref(pos, dict_idx);
    }

    fn on_string_start(&mut self, _sov: usize, _length: usize) {
        self.str_buf.clear();
    }

    fn on_string_fragment(&mut self, fragment: &[u8]) {
        self.str_buf.extend_from_slice(fragment);
    }

    fn on_string_end(&mut self) {
        let s = String::from_utf8_lossy(&self.str_buf).into_owned();
        self.inner.push_value(JsonValue::String(s));
    }
}

// ============================================================
// Encoder round-trip harness
// ============================================================

/// Multi-record encoder test helper
pub struct EncoderTestHarness {
    encoder: AuEncoder,
    storage: Vec<u8>,
}

impl EncoderTestHarness {
    pub fn new() -> Self {
        EncoderTestHarness {
            encoder: AuEncoder::new(),
            storage: Vec::new(),
        }
    }

    pub fn encode(&mut self, f: impl FnOnce(&mut AuWriter)) {
        let storage = &mut self.storage;
        self.encoder.encode(f, |s1, s2| {
            storage.extend_from_slice(s1);
            storage.extend_from_slice(s2);
            s1.len() + s2.len()
        });
    }

    pub fn get_json(&self) -> String {
        let mut source = BufferByteSource::new(&self.storage);
        let mut dictionary = Dictionary::new();

        let mut handler = MultiValueJsonHandler::new();
        let mut record_handler = AuRecordHandler::new(&mut dictionary, &mut handler);
        let _ = parse_stream(&mut source, &mut record_handler, true);
        drop(record_handler);
        handler.finalize();

        handler.results.join("\n")
    }
}

impl Default for EncoderTestHarness {
    fn default() -> Self {
        Self::new()
    }
}

use au::record_handler::AuRecordHandler;

/// Handler that collects multiple JSON values (one per au record)
pub struct MultiValueJsonHandler {
    inner: StringCollectingJsonHandler,
    results: Vec<String>,
}

impl MultiValueJsonHandler {
    pub const fn new() -> Self {
        MultiValueJsonHandler {
            inner: StringCollectingJsonHandler::new(),
            results: Vec::new(),
        }
    }

    fn finalize(&mut self) {
        if let Some(val) = self.inner.take_result() {
            self.results.push(serde_json::to_string(&val).unwrap());
        }
    }

    fn check_and_collect(&mut self) {
        if let Some(val) = self.inner.take_result() {
            self.results.push(serde_json::to_string(&val).unwrap());
        }
    }
}

impl Default for MultiValueJsonHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ValueHandler for MultiValueJsonHandler {
    fn on_object_start(&mut self) {
        self.check_and_collect();
        self.inner.on_object_start();
    }
    fn on_object_end(&mut self) {
        self.inner.on_object_end();
    }
    fn on_array_start(&mut self) {
        self.check_and_collect();
        self.inner.on_array_start();
    }
    fn on_array_end(&mut self) {
        self.inner.on_array_end();
    }
    fn on_null(&mut self, pos: usize) {
        self.check_and_collect();
        self.inner.on_null(pos);
    }
    fn on_bool(&mut self, pos: usize, val: bool) {
        self.check_and_collect();
        self.inner.on_bool(pos, val);
    }
    fn on_int(&mut self, pos: usize, val: i64) {
        self.check_and_collect();
        self.inner.on_int(pos, val);
    }
    fn on_uint(&mut self, pos: usize, val: u64) {
        self.check_and_collect();
        self.inner.on_uint(pos, val);
    }
    fn on_double(&mut self, pos: usize, val: f64) {
        self.check_and_collect();
        self.inner.on_double(pos, val);
    }
    fn on_time(&mut self, pos: usize, nanos: u64) {
        self.check_and_collect();
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

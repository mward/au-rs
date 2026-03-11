use crate::buffer::VectorBuffer;
use crate::byte_source::BufferByteSource;
use crate::common::*;
use crate::decoder::parse_stream;
use crate::dictionary::Dictionary;
use crate::encoder::AuEncoder;
use crate::handler::ValueHandler;
use crate::json_handler::StringCollectingJsonHandler;
use crate::record_handler::AuRecordHandler;
use crate::string_intern::{InternMode, StringIntern, StringInternConfig};
use crate::writer::AuWriter;

/// Multi-record encoder test helper
struct EncoderTestHarness {
    encoder: AuEncoder,
    storage: Vec<u8>,
}

impl EncoderTestHarness {
    fn new() -> Self {
        EncoderTestHarness {
            encoder: AuEncoder::new(),
            storage: Vec::new(),
        }
    }

    fn encode(&mut self, f: impl FnOnce(&mut AuWriter)) {
        let storage = &mut self.storage;
        self.encoder.encode(
            f,
            |s1, s2| {
                storage.extend_from_slice(s1);
                storage.extend_from_slice(s2);
                s1.len() + s2.len()
            },
        );
    }

    fn get_json(&self) -> String {
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

/// Handler that collects multiple JSON values (one per au record)
struct MultiValueJsonHandler {
    inner: StringCollectingJsonHandler,
    results: Vec<String>,
}

impl MultiValueJsonHandler {
    fn new() -> Self {
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

impl ValueHandler for MultiValueJsonHandler {
    fn on_object_start(&mut self) {
        self.check_and_collect();
        self.inner.on_object_start();
    }
    fn on_object_end(&mut self) { self.inner.on_object_end(); }
    fn on_array_start(&mut self) {
        self.check_and_collect();
        self.inner.on_array_start();
    }
    fn on_array_end(&mut self) { self.inner.on_array_end(); }
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
    fn on_string_end(&mut self) { self.inner.on_string_end(); }
    fn on_string_fragment(&mut self, fragment: &[u8]) {
        self.inner.on_string_fragment(fragment);
    }
}


// ============================================================
// AuStringIntern Tests (from AuUnitTests.cpp)
// ============================================================

#[test]
fn string_intern_no_intern() {
    let mut si = StringIntern::new();
    assert_eq!(0, si.dict().len());
    assert!(si.idx("shrt", InternMode::ByFrequency).is_none());
    assert!(si.idx("Long string", InternMode::ByFrequency).is_none());
    assert_eq!(0, si.dict().len());
}

#[test]
fn string_intern_force_intern() {
    let mut si = StringIntern::new();
    assert_eq!(0, si.dict().len());

    // Tiny strings are not interned even if forced
    assert!(si.idx("tiny", InternMode::ForceIntern).is_none());
    assert_eq!(0, si.dict().len());

    assert!(si.idx("A normal string", InternMode::ForceIntern).is_some());
    assert_eq!(1, si.dict().len());
}

#[test]
fn string_intern_frequent_strings() {
    const INTERN_THRESH: usize = 10;
    let mut si = StringIntern::with_config(StringInternConfig {
        tiny_str: 4,
        intern_thresh: INTERN_THRESH,
        ..Default::default()
    });
    let str_val = "Normal value";

    assert!(si.idx(str_val, InternMode::ByFrequency).is_none());
    assert_eq!(0, si.dict().len());

    for i in 0..(INTERN_THRESH * 2) {
        if i < INTERN_THRESH - 1 {
            assert!(
                si.idx(str_val, InternMode::ByFrequency).is_none(),
                "i = {}",
                i
            );
            assert_eq!(0, si.dict().len(), "i = {}", i);
        } else {
            assert!(si.idx(str_val, InternMode::ByFrequency).is_some());
            assert_eq!(1, si.dict().len());
        }
    }
}

#[test]
fn string_intern_reindex() {
    let mut si = StringIntern::with_config(StringInternConfig {
        tiny_str: 1,
        intern_thresh: 2,
        intern_cache_size: 10,
        ..Default::default()
    });

    si.idx("twice", InternMode::ForceIntern); // idx 0
    si.idx("once", InternMode::ForceIntern); // idx 1
    si.idx("thrice", InternMode::ForceIntern); // idx 2
    si.idx("twice", InternMode::ForceIntern);
    si.idx("thrice", InternMode::ForceIntern);
    si.idx("thrice", InternMode::ForceIntern);

    assert_eq!(3, si.dict().len());
    assert_eq!("twice", si.dict()[0]);
    assert_eq!("once", si.dict()[1]);
    assert_eq!("thrice", si.dict()[2]);

    assert_eq!(1, si.reindex(2));

    assert_eq!(2, si.dict().len());
    assert_eq!("thrice", si.dict()[0]);
    assert_eq!("twice", si.dict()[1]);

    assert_eq!(Some(0), si.idx("thrice", InternMode::ForceIntern));
    assert_eq!(Some(1), si.idx("twice", InternMode::ForceIntern));

    si.idx("quadrice", InternMode::ForceIntern);
    assert_eq!(Some(2), si.idx("quadrice", InternMode::ForceIntern));
}

// ============================================================
// AuFormatterTest (AuWriter) Tests (from AuUnitTests.cpp)
// ============================================================

fn make_writer() -> (VectorBuffer, StringIntern) {
    (VectorBuffer::new(), StringIntern::new())
}

#[test]
fn formatter_null() {
    let (mut buf, mut si) = make_writer();
    {
        let mut writer = AuWriter::new(&mut buf, &mut si);
        writer.null();
        writer.null(); // value(nullptr) equivalent
    }
    assert_eq!(&[0x00, 0x00], buf.as_bytes());
}

#[test]
fn formatter_bool() {
    let (mut buf, mut si) = make_writer();
    {
        let mut writer = AuWriter::new(&mut buf, &mut si);
        writer.value_bool(true);
        writer.value_bool(false);
    }
    assert_eq!(&[0x01, 0x02], buf.as_bytes());
}

#[test]
fn formatter_int() {
    let (mut buf, mut si) = make_writer();
    {
        let mut writer = AuWriter::new(&mut buf, &mut si);
        writer.value_i32(0).value_i32(127).value_i32(128);
        writer.value_i32(-1).value_i32(-127).value_i32(-128);
        writer.value_u32(0xff).value_u32(0x100);
    }
    let expected: Vec<u8> = vec![
        // Small positives
        SMALL_INT_POSITIVE | 0, // 0
        Marker::Varint as u8,
        127, // 127
        Marker::Varint as u8,
        0x80,
        0x01, // 128
        // Small negatives
        SMALL_INT_NEGATIVE | 1, // -1
        Marker::NegVarint as u8,
        127, // -127
        Marker::NegVarint as u8,
        0x80,
        0x01, // -128
        // Larger positives
        Marker::Varint as u8,
        0xff,
        0x01, // 0xff
        Marker::Varint as u8,
        0x80,
        0x02, // 0x100
    ];
    assert_eq!(expected, buf.as_bytes());
}

#[test]
fn formatter_int64() {
    let (mut buf, mut si) = make_writer();
    {
        let mut writer = AuWriter::new(&mut buf, &mut si);
        writer.value_i64(0x1234567890abcdef_i64);
        writer.value_i64(-0x1234567890abcdef_i64);
        writer.value_u64(0xf234567890abcdef_u64);
        writer.value_u64(0xffffffffffffffff_u64);
        let val: u64 = 9223372036856150856;
        writer.value_i64(val as i64);
        writer.value_u64(val);
        writer.value_i64(i64::MIN);
    }
    let expected: Vec<u8> = vec![
        Marker::PosInt64 as u8,
        0xef, 0xcd, 0xab, 0x90, 0x78, 0x56, 0x34, 0x12,
        Marker::NegInt64 as u8,
        0xef, 0xcd, 0xab, 0x90, 0x78, 0x56, 0x34, 0x12,
        Marker::PosInt64 as u8,
        0xef, 0xcd, 0xab, 0x90, 0x78, 0x56, 0x34, 0xf2,
        Marker::PosInt64 as u8,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        Marker::NegInt64 as u8,
        0xb8, 0x04, 0xeb, 0xff, 0xff, 0xff, 0xff, 0x7f,
        Marker::PosInt64 as u8,
        0x48, 0xfb, 0x14, 0x00, 0x00, 0x00, 0x00, 0x80,
        Marker::NegInt64 as u8,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80,
    ];
    assert_eq!(expected, buf.as_bytes());
}

#[test]
fn formatter_time() {
    let (mut buf, mut si) = make_writer();
    {
        let mut writer = AuWriter::new(&mut buf, &mut si);
        // 35 seconds in nanoseconds
        let nanos: u64 = 35_000_000_000;
        writer.nanos(nanos);
    }
    let expected: Vec<u8> = vec![
        0x04, 0x00, 0x9e, 0x29, 0x26, 0x08, 0x00, 0x00, 0x00,
    ];
    assert_eq!(expected, buf.as_bytes());
}

#[test]
fn formatter_double() {
    let (mut buf, mut si) = make_writer();
    {
        let mut writer = AuWriter::new(&mut buf, &mut si);
        writer.value_f64(5.9);
    }
    let expected: Vec<u8> = vec![0x03, 0x9A, 0x99, 0x99, 0x99, 0x99, 0x99, 0x17, 0x40];
    assert_eq!(expected, buf.as_bytes());
}

#[test]
fn formatter_float() {
    let (mut buf, mut si) = make_writer();
    {
        let mut writer = AuWriter::new(&mut buf, &mut si);
        writer.value_f32(5.9_f32);
    }
    let expected: Vec<u8> = vec![0x03, 0x00, 0x00, 0x00, 0xA0, 0x99, 0x99, 0x17, 0x40];
    assert_eq!(expected, buf.as_bytes());
}

#[test]
fn formatter_nan() {
    let (mut buf, mut si) = make_writer();
    {
        let mut writer = AuWriter::new(&mut buf, &mut si);
        writer.start_array();
        writer.value_f32(f32::NAN);
        writer.value_f64(f64::NAN);
        writer.value_f64(f64::NAN); // long double->double equivalent
        writer.value_f64(f64::from_bits(f64::NAN.to_bits() | (1u64 << 63))); // negative NaN
        writer.end_array();
    }
    let expected: Vec<u8> = vec![
        Marker::ArrayStart as u8,
        Marker::Double as u8, 0, 0, 0, 0, 0, 0, 0xf8, 0x7f,
        Marker::Double as u8, 0, 0, 0, 0, 0, 0, 0xf8, 0x7f,
        Marker::Double as u8, 0, 0, 0, 0, 0, 0, 0xf8, 0x7f,
        Marker::Double as u8, 0, 0, 0, 0, 0, 0, 0xf8, 0xff,
        Marker::ArrayEnd as u8,
    ];
    assert_eq!(expected, buf.as_bytes());
}

#[test]
fn formatter_inf() {
    let (mut buf, mut si) = make_writer();
    {
        let mut writer = AuWriter::new(&mut buf, &mut si);
        writer.start_array();
        writer.value_f64(f64::INFINITY);
        writer.value_f64(f64::NEG_INFINITY);
        writer.value_f32(f32::INFINITY);
        writer.value_f32(f32::NEG_INFINITY);
        writer.end_array();
    }
    let expected: Vec<u8> = vec![
        Marker::ArrayStart as u8,
        Marker::Double as u8, 0, 0, 0, 0, 0, 0, 0xf0, 0x7f,
        Marker::Double as u8, 0, 0, 0, 0, 0, 0, 0xf0, 0xff,
        Marker::Double as u8, 0, 0, 0, 0, 0, 0, 0xf0, 0x7f,
        Marker::Double as u8, 0, 0, 0, 0, 0, 0, 0xf0, 0xff,
        Marker::ArrayEnd as u8,
    ];
    assert_eq!(expected, buf.as_bytes());
}

#[test]
fn formatter_short_string() {
    let (mut buf, mut si) = make_writer();
    {
        let mut writer = AuWriter::new(&mut buf, &mut si);
        writer.value_str("str");
    }
    let expected: Vec<u8> = vec![0x20 | 3, b's', b't', b'r'];
    assert_eq!(expected, buf.as_bytes());
}

#[test]
fn formatter_long_string() {
    let (mut buf, mut si) = make_writer();
    {
        let mut writer = AuWriter::new(&mut buf, &mut si);
        writer.value_str("aLongerString, longer than 32 chars, the important thing");
    }
    let mut expected: Vec<u8> = vec![0x05, 0x38];
    expected.extend_from_slice(b"aLongerString, longer than 32 chars, the important thing");
    assert_eq!(expected, buf.as_bytes());
}

#[test]
fn formatter_intern_string() {
    let (mut buf, mut si) = make_writer();
    si.idx("aLongInternedString", InternMode::ForceIntern);
    si.idx("another string", InternMode::ForceIntern);
    {
        let mut writer = AuWriter::new(&mut buf, &mut si);
        writer.value_str_intern("aLongInternedString", InternMode::ForceIntern);
        writer.value_str_intern("another string", InternMode::ForceIntern);
    }
    let expected: Vec<u8> = vec![0x80 | 0, 0x80 | 1];
    assert_eq!(expected, buf.as_bytes());
}

#[test]
fn formatter_empty_map() {
    let (mut buf, mut si) = make_writer();
    {
        let mut writer = AuWriter::new(&mut buf, &mut si);
        writer.map(|_| {});
    }
    assert_eq!(&[0x0d, 0x0e], buf.as_bytes());
}

#[test]
fn formatter_flat_map() {
    let (mut buf, mut si) = make_writer();
    {
        let mut writer = AuWriter::new(&mut buf, &mut si);
        writer.map(|w| {
            w.kv_str("Key1", "value1");
            w.kv_str("key1", "Value1");
        });
    }
    let mut expected = vec![0x0du8];
    expected.push(0x24); expected.extend_from_slice(b"Key1");
    expected.push(0x26); expected.extend_from_slice(b"value1");
    expected.push(0x24); expected.extend_from_slice(b"key1");
    expected.push(0x26); expected.extend_from_slice(b"Value1");
    expected.push(0x0e);
    assert_eq!(expected, buf.as_bytes());
}

#[test]
fn formatter_nested_map() {
    let (mut buf, mut si) = make_writer();
    {
        let mut writer = AuWriter::new(&mut buf, &mut si);
        writer.map(|w| {
            w.kv_str("k1", "v1");
            // "nested" gets interned because key() forces intern, and it's > 4 chars
            w.key("nested");
            w.map(|w| {
                w.kv_str("k2", "v2");
            });
        });
    }
    let expected: Vec<u8> = vec![
        0x0d, 0x22, b'k', b'1', 0x22, b'v', b'1',
        0x80, // interned "nested" -> dict ref 0
        0x0d, 0x22, b'k', b'2', 0x22, b'v', b'2', 0x0e, 0x0e,
    ];
    assert_eq!(expected, buf.as_bytes());
}

#[test]
fn formatter_empty_array() {
    let (mut buf, mut si) = make_writer();
    {
        let mut writer = AuWriter::new(&mut buf, &mut si);
        writer.array(|_| {});
    }
    assert_eq!(&[0x0b, 0x0c], buf.as_bytes());
}

#[test]
fn formatter_flat_array() {
    let (mut buf, mut si) = make_writer();
    {
        let mut writer = AuWriter::new(&mut buf, &mut si);
        writer.array(|w| {
            w.value_i32(1);
            w.value_i32(2);
            w.value_i32(3);
        });
    }
    assert_eq!(&[0x0b, 0x61, 0x62, 0x63, 0x0c], buf.as_bytes());
}

#[test]
fn formatter_nested_array() {
    let (mut buf, mut si) = make_writer();
    {
        let mut writer = AuWriter::new(&mut buf, &mut si);
        writer.array(|w| {
            w.value_i32(1);
            w.value_i32(2);
            w.array(|w| {
                w.value_i32(3);
                w.value_i32(4);
            });
        });
    }
    assert_eq!(
        &[0x0b, 0x61, 0x62, 0x0b, 0x63, 0x64, 0x0c, 0x0c],
        buf.as_bytes()
    );
}

// ============================================================
// AuEncoderTest (round-trip) Tests (from AuEncoderTests.cpp)
// ============================================================

#[test]
fn encoder_creation() {
    let _au = AuEncoder::new();
}

#[test]
fn encoder_small_int() {
    let mut h = EncoderTestHarness::new();
    h.encode(|w| { w.value_i32(2); });
    assert_eq!("2", h.get_json());
}

#[test]
fn encoder_small_neg_int() {
    let mut h = EncoderTestHarness::new();
    h.encode(|w| { w.value_i32(-9); });
    assert_eq!("-9", h.get_json());
}

#[test]
fn encoder_big_neg_int() {
    let mut h = EncoderTestHarness::new();
    h.encode(|w| { w.value_i32(-99999); });
    assert_eq!("-99999", h.get_json());
}

#[test]
fn encoder_really_big_neg_int() {
    let mut h = EncoderTestHarness::new();
    h.encode(|w| { w.value_i64(i64::MIN); });
    assert_eq!("-9223372036854775808", h.get_json());
}

#[test]
fn encoder_big_int() {
    let mut h = EncoderTestHarness::new();
    h.encode(|w| { w.value_i32(299792458); });
    assert_eq!("299792458", h.get_json());
}

#[test]
fn encoder_empty_string() {
    let mut h = EncoderTestHarness::new();
    h.encode(|w| { w.value_str(""); });
    assert_eq!(r#""""#, h.get_json());
}

#[test]
fn encoder_array() {
    let mut h = EncoderTestHarness::new();
    h.encode(|w| {
        w.array(|w| {
            for v in &[1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144] {
                w.value_i32(*v);
            }
        });
    });
    assert_eq!("[1,1,2,3,5,8,13,21,34,55,89,144]", h.get_json());
}

#[test]
fn encoder_mixed_object_value() {
    let mut h = EncoderTestHarness::new();
    let vec = vec![1, 2, 3, 5, 7];
    h.encode(|w| {
        w.map(|w| {
            w.kv_str("SimpleKey", "AStringValue");
            w.key("listOfNumbers");
            w.array(|w| {
                for v in &vec {
                    w.value_i32(*v);
                }
            });
        });
    });
    assert_eq!(
        r#"{"SimpleKey":"AStringValue","listOfNumbers":[1,2,3,5,7]}"#,
        h.get_json()
    );
}

#[test]
fn encoder_mixed_object_value2() {
    let mut h = EncoderTestHarness::new();
    h.encode(|w| {
        w.map(|w| {
            w.kv_str("SimpleKey", "AStringValue");
            w.key("Numeric");
            w.value_i32(42);
        });
    });
    assert_eq!(
        r#"{"SimpleKey":"AStringValue","Numeric":42}"#,
        h.get_json()
    );
}

#[test]
fn encoder_mixed_object_map() {
    let mut h = EncoderTestHarness::new();
    let vec = vec![1, 5, 7];
    h.encode(|w| {
        w.map(|w| {
            w.kv_str("SimpleKey", "AStringValue");
            w.key("listOfObjects");
            w.array(|w| {
                for v in &vec {
                    w.map(|w| {
                        w.key("val");
                        w.value_i32(*v);
                    });
                }
            });
        });
    });
    assert_eq!(
        r#"{"SimpleKey":"AStringValue","listOfObjects":[{"val":1},{"val":5},{"val":7}]}"#,
        h.get_json()
    );
}

#[test]
fn encoder_multi_record() {
    let mut h = EncoderTestHarness::new();
    h.encode(|w| {
        w.map(|w| {
            w.kv_str("1st", "record");
            w.kv_f64("key", 3.141);
        });
    });
    h.encode(|w| {
        w.map(|w| {
            w.kv_str("2nd", "record");
            w.kv_f64("transcends", 2.71828);
        });
    });
    assert_eq!(
        "{\"1st\":\"record\",\"key\":3.141}\n{\"2nd\":\"record\",\"transcends\":2.71828}",
        h.get_json()
    );
}

//! AuEncoder round-trip tests (ported from AuEncoderTests.cpp).

mod common;

use au::encoder::AuEncoder;
use common::EncoderTestHarness;

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
            w.kv_f64("key", 3.125);
        });
    });
    h.encode(|w| {
        w.map(|w| {
            w.kv_str("2nd", "record");
            w.kv_f64("transcends", 2.625);
        });
    });
    assert_eq!(
        "{\"1st\":\"record\",\"key\":3.125}\n{\"2nd\":\"record\",\"transcends\":2.625}",
        h.get_json()
    );
}

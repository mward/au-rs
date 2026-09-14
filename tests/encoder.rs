//! AuEncoder round-trip tests (ported from AuEncoderTests.cpp).

mod common;

use au::encoder::AuEncoder;
use common::{EncoderTestHarness, decode_to_json_string};

#[test]
fn encoder_creation() {
    let _au = AuEncoder::new();
}

#[test]
fn encoder_small_int() {
    let mut h = EncoderTestHarness::new();
    h.encode(|w| {
        w.value_i32(2);
    });
    assert_eq!("2", h.get_json());
}

#[test]
fn encoder_small_neg_int() {
    let mut h = EncoderTestHarness::new();
    h.encode(|w| {
        w.value_i32(-9);
    });
    assert_eq!("-9", h.get_json());
}

#[test]
fn encoder_big_neg_int() {
    let mut h = EncoderTestHarness::new();
    h.encode(|w| {
        w.value_i32(-99999);
    });
    assert_eq!("-99999", h.get_json());
}

#[test]
fn encoder_really_big_neg_int() {
    let mut h = EncoderTestHarness::new();
    h.encode(|w| {
        w.value_i64(i64::MIN);
    });
    assert_eq!("-9223372036854775808", h.get_json());
}

#[test]
fn encoder_big_int() {
    let mut h = EncoderTestHarness::new();
    h.encode(|w| {
        w.value_i32(299792458);
    });
    assert_eq!("299792458", h.get_json());
}

#[test]
fn encoder_empty_string() {
    let mut h = EncoderTestHarness::new();
    h.encode(|w| {
        w.value_str("");
    });
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
    assert_eq!(r#"{"SimpleKey":"AStringValue","Numeric":42}"#, h.get_json());
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

// ============================================================
// Backref-overflow protection (ported from AuEncoderBackref.*)
// ============================================================

/// Encode `records` maps with a stable interned value under a given backref
/// threshold. Purge/reindex are disabled so only the backref threshold drives
/// dictionary records. Returns the encoded bytes.
fn encode_with_backref_threshold(threshold: usize, records: usize) -> Vec<u8> {
    let mut enc = AuEncoder::with_options(String::new(), 0, 50, 0);
    enc.set_backref_threshold(threshold);
    let mut storage = Vec::new();
    for n in 0..records as i64 {
        enc.encode(
            |w| {
                w.map(|w| {
                    w.kv_i64("n", n);
                    w.kv_str("stable", "a value long enough to get interned");
                });
            },
            |a, b| {
                storage.extend_from_slice(a);
                storage.extend_from_slice(b);
                a.len() + b.len()
            },
        );
    }
    storage
}

/// A small backref threshold forces extra dictionary records (larger output),
/// but the decoded content must be identical to the relaxed-threshold stream.
#[test]
fn extra_dictionary_records_do_not_break_decoding() {
    let relaxed = encode_with_backref_threshold(1 << 20, 500);
    let frequent = encode_with_backref_threshold(256, 500);

    assert!(
        frequent.len() > relaxed.len(),
        "a small threshold should have forced extra dictionary records \
         (frequent={}, relaxed={})",
        frequent.len(),
        relaxed.len()
    );
    assert_eq!(
        decode_to_json_string(&relaxed),
        decode_to_json_string(&frequent)
    );
}

/// `checked_backref` accepts values up to u32::MAX and returns them unchanged.
#[test]
fn narrowing_accepts_values_up_to_u32_max() {
    let limit = u32::MAX as usize;
    assert_eq!(0, AuEncoder::checked_backref(0));
    assert_eq!(u32::MAX, AuEncoder::checked_backref(limit));
}

/// `checked_backref` panics rather than silently truncating past u32::MAX.
#[test]
#[should_panic(expected = "exceeds 32 bits")]
fn narrowing_past_u32_max_panics() {
    let _ = AuEncoder::checked_backref(u32::MAX as usize + 1);
}

/// The default threshold leaves at least 1 GiB of headroom below the 2^32
/// limit, so a single large record can't push the backref past it after the
/// last check.
#[test]
fn default_threshold_leaves_room_for_a_large_record() {
    let limit: u64 = 1 << 32;
    let threshold = AuEncoder::DEFAULT_BACKREF_THRESHOLD as u64;
    assert!(threshold < limit);
    assert!(limit - threshold >= (1 << 30));
}

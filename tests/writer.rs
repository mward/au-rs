//! AuFormatter (AuWriter) tests (ported from AuUnitTests.cpp).

use au::buffer::VectorBuffer;
use au::common::*;
use au::string_intern::{InternMode, StringIntern};
use au::writer::AuWriter;

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
        SMALL_INT_POSITIVE, // 0
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
        0xef,
        0xcd,
        0xab,
        0x90,
        0x78,
        0x56,
        0x34,
        0x12,
        Marker::NegInt64 as u8,
        0xef,
        0xcd,
        0xab,
        0x90,
        0x78,
        0x56,
        0x34,
        0x12,
        Marker::PosInt64 as u8,
        0xef,
        0xcd,
        0xab,
        0x90,
        0x78,
        0x56,
        0x34,
        0xf2,
        Marker::PosInt64 as u8,
        0xff,
        0xff,
        0xff,
        0xff,
        0xff,
        0xff,
        0xff,
        0xff,
        Marker::NegInt64 as u8,
        0xb8,
        0x04,
        0xeb,
        0xff,
        0xff,
        0xff,
        0xff,
        0x7f,
        Marker::PosInt64 as u8,
        0x48,
        0xfb,
        0x14,
        0x00,
        0x00,
        0x00,
        0x00,
        0x80,
        Marker::NegInt64 as u8,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x80,
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
    let expected: Vec<u8> = vec![0x04, 0x00, 0x9e, 0x29, 0x26, 0x08, 0x00, 0x00, 0x00];
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
        Marker::Double as u8,
        0,
        0,
        0,
        0,
        0,
        0,
        0xf8,
        0x7f,
        Marker::Double as u8,
        0,
        0,
        0,
        0,
        0,
        0,
        0xf8,
        0x7f,
        Marker::Double as u8,
        0,
        0,
        0,
        0,
        0,
        0,
        0xf8,
        0x7f,
        Marker::Double as u8,
        0,
        0,
        0,
        0,
        0,
        0,
        0xf8,
        0xff,
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
        Marker::Double as u8,
        0,
        0,
        0,
        0,
        0,
        0,
        0xf0,
        0x7f,
        Marker::Double as u8,
        0,
        0,
        0,
        0,
        0,
        0,
        0xf0,
        0xff,
        Marker::Double as u8,
        0,
        0,
        0,
        0,
        0,
        0,
        0xf0,
        0x7f,
        Marker::Double as u8,
        0,
        0,
        0,
        0,
        0,
        0,
        0xf0,
        0xff,
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
    let expected: Vec<u8> = vec![0x80, 0x80 | 1];
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
    expected.push(0x24);
    expected.extend_from_slice(b"Key1");
    expected.push(0x26);
    expected.extend_from_slice(b"value1");
    expected.push(0x24);
    expected.extend_from_slice(b"key1");
    expected.push(0x26);
    expected.extend_from_slice(b"Value1");
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
        0x0d, 0x22, b'k', b'1', 0x22, b'v', b'1', 0x80, // interned "nested" -> dict ref 0
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

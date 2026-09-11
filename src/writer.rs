use crate::buffer::VectorBuffer;
use crate::common::*;
use crate::string_intern::{InternMode, StringIntern};

pub struct AuWriter<'a> {
    msg_buf: &'a mut VectorBuffer,
    string_intern: &'a mut StringIntern,
}

impl<'a> AuWriter<'a> {
    #[inline]
    pub const fn new(buf: &'a mut VectorBuffer, string_intern: &'a mut StringIntern) -> Self {
        AuWriter {
            msg_buf: buf,
            string_intern,
        }
    }

    #[inline]
    pub const fn msg_buf_tellp(&self) -> usize {
        self.msg_buf.tellp()
    }

    fn encode_string(&mut self, sv: &str) {
        write_explicit_string(self.msg_buf, sv);
    }

    fn encode_string_intern(&mut self, sv: &str, intern: InternMode) {
        let idx = self.string_intern.idx(sv, intern);
        match idx {
            None => self.encode_string(sv),
            Some(i) if i < 0x80 => {
                self.msg_buf.put(0x80 | i as u8);
            }
            Some(i) => {
                self.msg_buf.put(Marker::DictRef as u8);
                self.value_int(i as u64);
            }
        }
    }

    // Public value methods

    #[inline]
    pub fn null(&mut self) -> &mut Self {
        self.msg_buf.put(Marker::Null as u8);
        self
    }

    #[inline]
    pub fn value_bool(&mut self, b: bool) -> &mut Self {
        self.msg_buf.put(if b {
            Marker::True as u8
        } else {
            Marker::False as u8
        });
        self
    }

    #[inline]
    pub fn value_str(&mut self, sv: &str) -> &mut Self {
        self.value_str_intern(sv, InternMode::ByFrequency)
    }

    #[inline]
    pub fn value_str_intern(&mut self, sv: &str, intern: InternMode) -> &mut Self {
        self.encode_string_intern(sv, intern);
        self
    }

    #[inline]
    pub fn value_i64(&mut self, i: i64) -> &mut Self {
        if (0..32).contains(&i) {
            self.msg_buf.put(SMALL_INT_POSITIVE | i as u8);
            return self;
        }
        if i < 0 && i > -32 {
            self.msg_buf.put(SMALL_INT_NEGATIVE | (-i) as u8);
            return self;
        }
        let neg = i < 0;
        // `unsigned_abs` yields the magnitude and handles i64::MIN without overflow.
        let val: u64 = i.unsigned_abs();
        if val >= 1u64 << 48 {
            self.msg_buf.put(if neg {
                Marker::NegInt64 as u8
            } else {
                Marker::PosInt64 as u8
            });
            self.msg_buf.write_bytes(&val.to_le_bytes());
            return self;
        }
        self.msg_buf.put(if neg {
            Marker::NegVarint as u8
        } else {
            Marker::Varint as u8
        });
        self.value_int(val);
        self
    }

    #[inline]
    pub fn value_u64(&mut self, i: u64) -> &mut Self {
        if i < 32 {
            self.msg_buf.put(SMALL_INT_POSITIVE | i as u8);
        } else if i >= 1u64 << 48 {
            self.msg_buf.put(Marker::PosInt64 as u8);
            self.msg_buf.write_bytes(&i.to_le_bytes());
        } else {
            self.msg_buf.put(Marker::Varint as u8);
            self.value_int(i);
        }
        self
    }

    #[inline]
    pub fn value_i32(&mut self, i: i32) -> &mut Self {
        self.value_i64(i as i64)
    }

    #[inline]
    pub fn value_u32(&mut self, i: u32) -> &mut Self {
        self.value_u64(i as u64)
    }

    #[inline]
    pub fn value_f64(&mut self, d: f64) -> &mut Self {
        self.msg_buf.put(Marker::Double as u8);
        self.msg_buf.write_bytes(&d.to_le_bytes());
        self
    }

    #[inline]
    pub fn value_f32(&mut self, f: f32) -> &mut Self {
        self.value_f64(f as f64)
    }

    #[inline]
    pub fn nanos(&mut self, n: u64) -> &mut Self {
        self.msg_buf.put(Marker::Timestamp as u8);
        self.msg_buf.write_bytes(&n.to_le_bytes());
        self
    }

    // Map and array methods

    #[inline]
    pub fn start_map(&mut self) -> &mut Self {
        self.msg_buf.put(Marker::ObjectStart as u8);
        self
    }

    #[inline]
    pub fn end_map(&mut self) -> &mut Self {
        self.msg_buf.put(Marker::ObjectEnd as u8);
        self
    }

    #[inline]
    pub fn start_array(&mut self) -> &mut Self {
        self.msg_buf.put(Marker::ArrayStart as u8);
        self
    }

    #[inline]
    pub fn end_array(&mut self) -> &mut Self {
        self.msg_buf.put(Marker::ArrayEnd as u8);
        self
    }

    #[inline]
    pub fn key(&mut self, k: &str) {
        self.encode_string_intern(k, InternMode::ForceIntern);
    }

    /// Convenience: write a map with key-value pairs via closure
    #[inline]
    pub fn map(&mut self, f: impl FnOnce(&mut AuWriter)) -> &mut Self {
        self.msg_buf.put(Marker::ObjectStart as u8);
        f(self);
        self.msg_buf.put(Marker::ObjectEnd as u8);
        self
    }

    /// Convenience: write an array with values via closure
    #[inline]
    pub fn array(&mut self, f: impl FnOnce(&mut AuWriter)) -> &mut Self {
        self.msg_buf.put(Marker::ArrayStart as u8);
        f(self);
        self.msg_buf.put(Marker::ArrayEnd as u8);
        self
    }

    /// Write a key-value pair
    #[inline]
    pub fn kv_str(&mut self, k: &str, v: &str) {
        self.key(k);
        self.value_str(v);
    }

    #[inline]
    pub fn kv_i64(&mut self, k: &str, v: i64) {
        self.key(k);
        self.value_i64(v);
    }

    #[inline]
    pub fn kv_u64(&mut self, k: &str, v: u64) {
        self.key(k);
        self.value_u64(v);
    }

    #[inline]
    pub fn kv_f64(&mut self, k: &str, v: f64) {
        self.key(k);
        self.value_f64(v);
    }

    #[inline]
    pub fn kv_bool(&mut self, k: &str, v: bool) {
        self.key(k);
        self.value_bool(v);
    }

    // Internal methods used by encoder

    #[inline]
    pub(crate) fn raw(&mut self, c: u8) {
        self.msg_buf.put(c);
    }

    #[inline]
    pub(crate) fn backref(&mut self, val: u32) {
        self.msg_buf.write_bytes(&val.to_le_bytes());
    }

    #[inline]
    pub(crate) fn value_int(&mut self, i: u64) {
        write_varint(self.msg_buf, i);
    }

    #[inline]
    pub(crate) fn term(&mut self) {
        write_term(self.msg_buf);
    }
}

/// Encode `i` as a little-endian base-128 varint into `buf`.
#[inline]
pub(crate) fn write_varint(buf: &mut VectorBuffer, mut i: u64) {
    loop {
        let to_write = (i & 0x7f) as u8;
        i >>= 7;
        if i != 0 {
            buf.put(to_write | 0x80);
        } else {
            buf.put(to_write);
            break;
        }
    }
}

/// Encode `sv` as an explicit (non-interned) string into `buf`.
#[inline]
pub(crate) fn write_explicit_string(buf: &mut VectorBuffer, sv: &str) {
    const MAX_INLINE_STRING_SIZE: usize = 31;
    let bytes = sv.as_bytes();
    if bytes.len() <= MAX_INLINE_STRING_SIZE {
        buf.put(0x20 | bytes.len() as u8);
    } else {
        buf.put(Marker::String as u8);
        write_varint(buf, bytes.len() as u64);
    }
    buf.write_bytes(bytes);
}

/// Write the record terminator (record-end marker + newline) into `buf`.
#[inline]
pub(crate) fn write_term(buf: &mut VectorBuffer) {
    buf.put(Marker::RecordEnd as u8);
    buf.put(b'\n');
}

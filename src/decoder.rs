use crate::byte_source::BufferByteSource;
use crate::common::*;
use crate::error::ParseError;
use crate::handler::{RecordHandler, ValueHandler};

const MAX_DEPTH: usize = 2048;

fn expect(source: &mut BufferByteSource, expected: u8) -> Result<(), ParseError> {
    let c = source.next();
    if c.is_eof() {
        return Err(ParseError::new(format!(
            "Unexpected EOF, expected 0x{:02x}",
            expected
        )));
    }
    if c.value() != expected {
        return Err(ParseError::new(format!(
            "Unexpected character: 0x{:02x}, expected 0x{:02x}",
            c.value(),
            expected
        )));
    }
    Ok(())
}

fn read_backref(source: &mut BufferByteSource) -> Result<u32, ParseError> {
    let bytes = source.read_bytes(4)?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_le_u64(source: &mut BufferByteSource) -> Result<u64, ParseError> {
    let bytes = source.read_bytes(8)?;
    Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_double(source: &mut BufferByteSource) -> Result<f64, ParseError> {
    let bytes = source.read_bytes(8)?;
    Ok(f64::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_varint(source: &mut BufferByteSource) -> Result<u64, ParseError> {
    let mut shift = 0u32;
    let mut result: u64 = 0;
    loop {
        if shift >= 64 {
            return Err(ParseError::new("Bad varint encoding"));
        }
        let next = source.next();
        if next.is_eof() {
            return Err(ParseError::new("Unexpected end of file"));
        }
        let i = next.value();
        result |= ((i & 0x7f) as u64) << shift;
        shift += 7;
        if (i & 0x80) == 0 {
            break;
        }
    }
    Ok(result)
}

fn parse_format_version(source: &mut BufferByteSource) -> Result<u64, ParseError> {
    let c = source.next();
    if c.is_eof() {
        return Err(ParseError::new("Expected version number, got EOF"));
    }
    let version;
    if (c.value() & !0x1f) == SMALL_INT_POSITIVE {
        version = (c.value() & 0x1f) as u64;
    } else if c.value() == Marker::Varint as u8 {
        version = read_varint(source)?;
    } else {
        return Err(ParseError::new("Expected version number"));
    }

    if version != AU_FORMAT_VERSION as u64 {
        return Err(ParseError::new(format!(
            "Bad format version: expected {}, got {}",
            AU_FORMAT_VERSION, version
        )));
    }
    Ok(version)
}

fn parse_string_data<H: ValueHandler>(
    source: &mut BufferByteSource,
    pos: usize,
    len: usize,
    handler: &mut H,
) -> Result<(), ParseError> {
    handler.on_string_start(pos, len);
    let data = source.read_bytes(len)?;
    handler.on_string_fragment(data);
    handler.on_string_end();
    Ok(())
}

fn parse_string_with_varint_len<H: ValueHandler>(
    source: &mut BufferByteSource,
    pos: usize,
    handler: &mut H,
) -> Result<(), ParseError> {
    let len = read_varint(source)? as usize;
    parse_string_data(source, pos, len, handler)
}

/// Parse the length of a full string from the tag byte already read.
fn parse_string_length(source: &mut BufferByteSource, tag: u8) -> Result<usize, ParseError> {
    if (tag & !0x1f) == 0x20 {
        Ok((tag & 0x1f) as usize)
    } else if tag == Marker::String as u8 {
        Ok(read_varint(source)? as usize)
    } else {
        Err(ParseError::new(format!(
            "Expected a string, got 0x{:02x}",
            tag
        )))
    }
}

fn parse_full_string_record(
    source: &mut BufferByteSource,
    handler: &mut impl RecordHandler,
) -> Result<(), ParseError> {
    let sov = source.pos();
    let c = source.next();
    if c.is_eof() {
        return Err(ParseError::new("Expected a string, got EOF"));
    }
    let len = parse_string_length(source, c.value())?;
    handler.on_string_start(sov, len);
    let data = source.read_bytes(len)?;
    handler.on_string_fragment(data);
    handler.on_string_end();
    Ok(())
}

/// Parse a single value from the source and dispatch to the handler
pub fn parse_value<H: ValueHandler>(
    source: &mut BufferByteSource,
    handler: &mut H,
    depth: usize,
) -> Result<(), ParseError> {
    if depth > MAX_DEPTH {
        return Err(ParseError::new("File too deeply nested"));
    }

    let sov = source.pos();
    let c = source.next();
    if c.is_eof() {
        return Err(ParseError::new("Unexpected EOF at start of value"));
    }

    // High bit set -> small dict ref
    if c.value() & 0x80 != 0 {
        handler.on_dict_ref(sov, (c.value() & !0x80) as usize);
        return Ok(());
    }

    let val = c.value() & !0xe0;
    if c.value() & SMALL_INT_NEGATIVE != 0 {
        if c.value() & 0x20 != 0 {
            // Positive small int
            handler.on_uint(sov, val as u64);
        } else {
            // Negative small int
            handler.on_int(sov, -(val as i64));
        }
        return Ok(());
    }
    if c.value() & 0x20 != 0 {
        // Inline string
        parse_string_data(source, sov, val as usize, handler)?;
        return Ok(());
    }

    match c.value() {
        x if x == Marker::True as u8 => handler.on_bool(sov, true),
        x if x == Marker::False as u8 => handler.on_bool(sov, false),
        x if x == Marker::Null as u8 => handler.on_null(sov),
        x if x == Marker::Varint as u8 => {
            let v = read_varint(source)?;
            handler.on_uint(sov, v);
        }
        x if x == Marker::NegVarint as u8 => {
            let v = read_varint(source)?;
            let neg_int_limit: u64 = (i64::MAX as u64) + 1;
            if v > neg_int_limit {
                return Err(ParseError::new(format!(
                    "Signed int overflows i64: (-){} 0x{:016x}",
                    v, v
                )));
            }
            handler.on_int(sov, -(v as i64));
        }
        x if x == Marker::PosInt64 as u8 => {
            let val = read_le_u64(source)?;
            handler.on_uint(sov, val);
        }
        x if x == Marker::NegInt64 as u8 => {
            let val = read_le_u64(source)?;
            let neg_int_limit: u64 = (i64::MAX as u64) + 1;
            if val > neg_int_limit {
                return Err(ParseError::new(format!(
                    "Signed int overflows i64: (-){} 0x{:016x}",
                    val, val
                )));
            }
            // 0 should be encoded as PosInt64, so val >= 1 for NegInt64
            // Use wrapping arithmetic to handle i64::MIN correctly
            handler.on_int(sov, (val as i64).wrapping_neg());
        }
        x if x == Marker::Double as u8 => {
            let v = read_double(source)?;
            handler.on_double(sov, v);
        }
        x if x == Marker::Timestamp as u8 => {
            let v = read_le_u64(source)?;
            handler.on_time(sov, v);
        }
        x if x == Marker::DictRef as u8 => {
            let v = read_varint(source)? as usize;
            handler.on_dict_ref(sov, v);
        }
        x if x == Marker::String as u8 => {
            parse_string_with_varint_len(source, sov, handler)?;
        }
        x if x == Marker::ArrayStart as u8 => {
            handler.on_array_start();
            loop {
                let p = source.peek();
                if p == Marker::ArrayEnd as u8 { break; }
                if p.is_eof() {
                    return Err(ParseError::new("Unexpected EOF in array"));
                }
                parse_value(source, handler, depth + 1)?;
            }
            expect(source, Marker::ArrayEnd as u8)?;
            handler.on_array_end();
        }
        x if x == Marker::ObjectStart as u8 => {
            handler.on_object_start();
            loop {
                let p = source.peek();
                if p == Marker::ObjectEnd as u8 { break; }
                if p.is_eof() {
                    return Err(ParseError::new("Unexpected EOF in object"));
                }
                parse_key(source, handler)?;
                parse_value(source, handler, depth + 1)?;
            }
            expect(source, Marker::ObjectEnd as u8)?;
            handler.on_object_end();
        }
        other => {
            return Err(ParseError::new(format!(
                "Unexpected character at start of value: 0x{:02x}",
                other
            )));
        }
    }
    Ok(())
}

fn parse_key<H: ValueHandler>(
    source: &mut BufferByteSource,
    handler: &mut H,
) -> Result<(), ParseError> {
    let sov = source.pos();
    let c = source.next();
    if c.is_eof() {
        return Err(ParseError::new("Unexpected EOF at start of key"));
    }
    if c.value() & 0x80 != 0 {
        handler.on_dict_ref(sov, (c.value() & !0x80) as usize);
        return Ok(());
    }
    let val = c.value() & !0xe0;
    if (c.value() & !0x1f) == 0x20 {
        parse_string_data(source, sov, val as usize, handler)?;
        return Ok(());
    }
    match c.value() {
        x if x == Marker::DictRef as u8 => {
            let v = read_varint(source)? as usize;
            handler.on_dict_ref(sov, v);
        }
        x if x == Marker::String as u8 => {
            parse_string_with_varint_len(source, sov, handler)?;
        }
        other => {
            return Err(ParseError::new(format!(
                "Unexpected character at start of key: 0x{:02x}",
                other
            )));
        }
    }
    Ok(())
}

fn term(source: &mut BufferByteSource) -> Result<(), ParseError> {
    expect(source, Marker::RecordEnd as u8)?;
    expect(source, b'\n')
}

/// Parse a full string from the source (used for header metadata and dict entries)
fn parse_full_string_to_string(
    source: &mut BufferByteSource,
    max_len: usize,
) -> Result<String, ParseError> {
    let c = source.next();
    if c.is_eof() {
        return Err(ParseError::new("Expected a string, got EOF"));
    }
    let len = parse_string_length(source, c.value())?;
    if len > max_len {
        return Err(ParseError::new("String too long"));
    }
    let data = source.read_bytes(len)?;
    Ok(String::from_utf8_lossy(data).into_owned())
}

/// Parse a record from the source
pub fn parse_record<H: RecordHandler>(
    source: &mut BufferByteSource,
    handler: &mut H,
) -> Result<bool, ParseError> {
    let c = source.next();
    if c.is_eof() {
        return Err(ParseError::new("Unexpected EOF at start of record"));
    }
    handler.on_record_start(source.pos() - 1);

    match c.value() {
        b'H' => {
            expect(source, b'A')?;
            expect(source, b'U')?;
            let version = parse_format_version(source)?;
            let metadata = parse_full_string_to_string(source, MAX_METADATA_SIZE)?;
            handler.on_header(version, &metadata);
            term(source)?;
            Ok(false)
        }
        b'C' => {
            parse_format_version(source)?;
            term(source)?;
            handler.on_dict_clear();
            Ok(false)
        }
        b'A' => {
            let backref = read_backref(source)?;
            handler.on_dict_add_start(backref as usize);
            while source.peek() != Marker::RecordEnd as u8 {
                if source.peek().is_eof() {
                    return Err(ParseError::new("Unexpected EOF in dict add"));
                }
                parse_full_string_record(source, handler)?;
            }
            term(source)?;
            Ok(false)
        }
        b'V' => {
            let backref = read_backref(source)?;
            let len = read_varint(source)? as usize;
            let start_of_value = source.pos();
            handler.on_value(backref as usize, len - 2, source);
            term(source)?;
            if source.pos() - start_of_value != len {
                return Err(ParseError::new(
                    "could be a parse error, or internal error: value handler didn't skip value!",
                ));
            }
            Ok(true)
        }
        other => Err(ParseError::new(format!(
            "Unexpected character at start of record: 0x{:02x}",
            other
        ))),
    }
}

/// Parse the full stream
pub fn parse_stream<H: RecordHandler>(
    source: &mut BufferByteSource,
    handler: &mut H,
    expect_header: bool,
) -> Result<(), ParseError> {
    if expect_header {
        check_header(source)?;
    }
    while !source.peek().is_eof() {
        parse_record(source, handler)?;
    }
    Ok(())
}

fn check_header(source: &mut BufferByteSource) -> Result<(), ParseError> {
    if source.peek().is_eof() {
        return Ok(());
    }

    if source.peek().value() != b'H' {
        return Err(ParseError::new(
            "This file doesn't appear to start with an au header record",
        ));
    }

    struct HeaderCheck {
        header_seen: bool,
    }
    impl RecordHandler for HeaderCheck {
        fn on_header(&mut self, _version: u64, _metadata: &str) {
            self.header_seen = true;
        }
    }

    let mut hh = HeaderCheck { header_seen: false };
    let _ = parse_record(source, &mut hh);
    if !hh.header_seen {
        return Err(ParseError::new(
            "This file doesn't appear to start with an au header record",
        ));
    }
    Ok(())
}

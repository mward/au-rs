use crate::error::ParseError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Byte {
    value: i16,
}

impl Byte {
    pub fn new(c: u8) -> Self {
        Byte { value: c as i16 }
    }

    pub fn eof() -> Self {
        Byte { value: -1 }
    }

    pub fn is_eof(&self) -> bool {
        self.value == -1
    }

    pub fn value(&self) -> u8 {
        assert!(!self.is_eof(), "Tried to get value of eof");
        self.value as u8
    }
}

impl PartialEq<u8> for Byte {
    fn eq(&self, other: &u8) -> bool {
        self.value == *other as i16
    }
}

pub struct BufferByteSource<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> BufferByteSource<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        BufferByteSource { buf, pos: 0 }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn end_pos(&self) -> usize {
        self.buf.len()
    }

    pub fn peek(&self) -> Byte {
        if self.pos < self.buf.len() {
            Byte::new(self.buf[self.pos])
        } else {
            Byte::eof()
        }
    }

    pub fn next(&mut self) -> Byte {
        if self.pos < self.buf.len() {
            let b = Byte::new(self.buf[self.pos]);
            self.pos += 1;
            b
        } else {
            Byte::eof()
        }
    }

    pub fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], ParseError> {
        let end = self.pos + len;
        if end > self.buf.len() {
            return Err(ParseError::new(format!(
                "read_bytes: not enough data, need {} more bytes",
                end - self.buf.len()
            )));
        }
        let slice = &self.buf[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    pub fn skip(&mut self, len: usize) -> Result<(), ParseError> {
        let end = self.pos + len;
        if end > self.buf.len() {
            return Err(ParseError::new("skip: not enough data"));
        }
        self.pos = end;
        Ok(())
    }
}

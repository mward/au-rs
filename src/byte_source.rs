use crate::error::ParseError;

pub struct BufferByteSource<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> BufferByteSource<'a> {
    #[inline]
    pub const fn new(buf: &'a [u8]) -> Self {
        BufferByteSource { buf, pos: 0 }
    }

    #[inline]
    pub const fn pos(&self) -> usize {
        self.pos
    }

    #[inline]
    pub const fn end_pos(&self) -> usize {
        self.buf.len()
    }

    /// Return the next byte without consuming it, or `None` at end of input.
    #[inline]
    pub fn peek(&self) -> Option<u8> {
        self.buf.get(self.pos).copied()
    }

    /// Consume and return the next byte, or `None` at end of input.
    #[allow(clippy::should_implement_trait)]
    #[inline]
    pub fn next(&mut self) -> Option<u8> {
        let b = self.buf.get(self.pos).copied()?;
        self.pos += 1;
        Some(b)
    }

    #[inline]
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

    #[inline]
    pub fn skip(&mut self, len: usize) -> Result<(), ParseError> {
        let end = self.pos + len;
        if end > self.buf.len() {
            return Err(ParseError::new("skip: not enough data"));
        }
        self.pos = end;
        Ok(())
    }
}

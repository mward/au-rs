use crate::error::ParseError;

pub struct BufferByteSource<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> BufferByteSource<'a> {
    #[inline]
    #[must_use]
    pub const fn new(buf: &'a [u8]) -> Self {
        BufferByteSource { buf, pos: 0 }
    }

    #[inline]
    #[must_use]
    pub const fn pos(&self) -> usize {
        self.pos
    }

    #[inline]
    #[must_use]
    pub const fn end_pos(&self) -> usize {
        self.buf.len()
    }

    /// Return the next byte without consuming it, or `None` at end of input.
    #[inline]
    #[must_use]
    pub fn peek(&self) -> Option<u8> {
        self.buf.get(self.pos).copied()
    }

    /// Consume and return the next byte, or `None` at end of input.
    #[inline]
    pub fn next_byte(&mut self) -> Option<u8> {
        let b = self.buf.get(self.pos).copied()?;
        self.pos += 1;
        Some(b)
    }

    #[inline]
    pub fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], ParseError> {
        // `self.pos <= self.buf.len()` always holds, so this slice never panics.
        let remaining = &self.buf[self.pos..];
        let slice = remaining.get(..len).ok_or_else(|| {
            ParseError::new(format!(
                "read_bytes: not enough data, need {} more bytes",
                len - remaining.len()
            ))
        })?;
        self.pos += len;
        Ok(slice)
    }

    #[inline]
    pub fn skip(&mut self, len: usize) -> Result<(), ParseError> {
        if len > self.buf.len() - self.pos {
            return Err(ParseError::new("skip: not enough data"));
        }
        self.pos += len;
        Ok(())
    }
}

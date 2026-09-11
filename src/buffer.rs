/// A growable output buffer for encoded bytes.
///
/// Thin wrapper over `Vec<u8>`; `clear` retains the allocation so the buffer can
/// be reused across records without reallocating.
pub struct VectorBuffer {
    v: Vec<u8>,
}

impl VectorBuffer {
    pub fn new() -> Self {
        Self::with_capacity(1024 * 1024)
    }

    pub fn with_capacity(size: usize) -> Self {
        VectorBuffer {
            v: Vec::with_capacity(size),
        }
    }

    pub fn put(&mut self, c: u8) {
        self.v.push(c);
    }

    /// Reserve `size` zeroed bytes at the end and return them for in-place writing.
    pub fn raw(&mut self, size: usize) -> &mut [u8] {
        let start = self.v.len();
        self.v.resize(start + size, 0);
        &mut self.v[start..]
    }

    pub fn write_bytes(&mut self, data: &[u8]) {
        self.v.extend_from_slice(data);
    }

    pub const fn tellp(&self) -> usize {
        self.v.len()
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.v
    }

    pub fn clear(&mut self) {
        self.v.clear();
    }
}

impl Default for VectorBuffer {
    fn default() -> Self {
        Self::new()
    }
}

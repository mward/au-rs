pub struct VectorBuffer {
    v: Vec<u8>,
    idx: usize,
}

impl VectorBuffer {
    pub fn new() -> Self {
        Self::with_capacity(1024 * 1024)
    }

    pub fn with_capacity(size: usize) -> Self {
        VectorBuffer {
            v: vec![0u8; size],
            idx: 0,
        }
    }

    pub fn put(&mut self, c: u8) {
        if self.idx == self.v.len() {
            self.v.resize(self.v.len() * 2, 0);
        }
        self.v[self.idx] = c;
        self.idx += 1;
    }

    pub fn raw(&mut self, size: usize) -> &mut [u8] {
        if self.idx + size > self.v.len() {
            let new_size = std::cmp::max(self.v.len() * 2, self.idx + size);
            self.v.resize(new_size, 0);
        }
        let front = self.idx;
        self.idx += size;
        &mut self.v[front..front + size]
    }

    pub fn write_bytes(&mut self, data: &[u8]) {
        if !data.is_empty() {
            let dest = self.raw(data.len());
            dest.copy_from_slice(data);
        }
    }

    pub fn tellp(&self) -> usize {
        self.idx
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.v[..self.idx]
    }

    pub fn clear(&mut self) {
        self.idx = 0;
    }
}

impl Default for VectorBuffer {
    fn default() -> Self {
        Self::new()
    }
}

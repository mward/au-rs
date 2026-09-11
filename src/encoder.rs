use crate::buffer::VectorBuffer;
use crate::common::*;
use crate::string_intern::{InternMode, StringIntern, StringInternConfig};
use crate::writer::{write_explicit_string, write_term, AuWriter};

pub struct AuEncoder {
    string_intern: StringIntern,
    dict_buf: VectorBuffer,
    buf: VectorBuffer,
    backref: usize,
    last_dict_size: usize,
    records: usize,
    purge_interval: usize,
    purge_threshold: usize,
    reindex_interval: usize,
    clear_threshold: usize,
}

impl AuEncoder {
    pub fn new() -> Self {
        Self::with_options(String::new(), 250_000, 50, 500_000)
    }

    pub fn with_metadata(metadata: String) -> Self {
        Self::with_options(metadata, 250_000, 50, 500_000)
    }

    pub fn with_options(
        metadata: String,
        purge_interval: usize,
        purge_threshold: usize,
        reindex_interval: usize,
    ) -> Self {
        Self::with_full_options(
            metadata,
            purge_interval,
            purge_threshold,
            reindex_interval,
            StringInternConfig::default(),
        )
    }

    pub fn with_full_options(
        mut metadata: String,
        purge_interval: usize,
        purge_threshold: usize,
        reindex_interval: usize,
        string_intern_config: StringInternConfig,
    ) -> Self {
        let clear_threshold = string_intern_config.clear_threshold;
        let mut encoder = AuEncoder {
            string_intern: StringIntern::with_config(string_intern_config),
            dict_buf: VectorBuffer::new(),
            buf: VectorBuffer::new(),
            backref: 0,
            last_dict_size: 0,
            records: 0,
            purge_interval,
            purge_threshold,
            reindex_interval,
            clear_threshold,
        };

        if metadata.len() > MAX_METADATA_SIZE {
            metadata.truncate(MAX_METADATA_SIZE);
        }

        // Write header
        {
            let mut af = AuWriter::new(&mut encoder.dict_buf, &mut encoder.string_intern);
            af.raw(b'H');
            af.raw(b'A');
            af.raw(b'U');
            af.value_u32(AU_FORMAT_VERSION);
            af.value_str_intern(&metadata, InternMode::ForceExplicit);
            af.term();
        }

        encoder.clear_dictionary(false);
        encoder
    }

    fn export_dict(&mut self) {
        let dict_len = self.string_intern.dict().len();
        if dict_len > self.last_dict_size {
            let sor = self.dict_buf.tellp();
            // `dict_buf` and `string_intern` are disjoint fields, so we can read the
            // interned strings while writing them out (as explicit, non-interned strings).
            self.dict_buf.put(b'A');
            self.dict_buf.write_bytes(&(self.backref as u32).to_le_bytes());
            for s in &self.string_intern.dict()[self.last_dict_size..dict_len] {
                write_explicit_string(&mut self.dict_buf, s);
            }
            write_term(&mut self.dict_buf);
            self.backref = self.dict_buf.tellp() - sor;
            self.last_dict_size = dict_len;
        }
    }

    fn finalize_and_write<W>(&mut self, write: W) -> isize
    where
        W: FnOnce(&[u8], &[u8]) -> usize,
    {
        self.export_dict();
        let sor = self.dict_buf.tellp();
        {
            let mut af = AuWriter::new(&mut self.dict_buf, &mut self.string_intern);
            af.raw(b'V');
            af.backref(self.backref as u32);
            af.value_int(self.buf.tellp() as u64);
        }
        self.backref += self.dict_buf.tellp() - sor;

        let result = write(self.dict_buf.as_bytes(), self.buf.as_bytes());

        self.records += 1;
        self.backref += self.buf.tellp();

        self.buf.clear();
        self.dict_buf.clear();

        if self.reindex_interval > 0 && self.records.is_multiple_of(self.reindex_interval) {
            self.reindex_dictionary(self.purge_threshold);
        }

        if self.purge_interval > 0
            && self.records.is_multiple_of(self.purge_interval)
            && self.last_dict_size > 0
        {
            self.purge_dictionary(self.purge_threshold);
        }

        if self.last_dict_size > self.clear_threshold {
            self.clear_dictionary(true);
        }

        result as isize
    }

    pub fn encode<F, W>(&mut self, f: F, write: W) -> isize
    where
        F: FnOnce(&mut AuWriter),
        W: FnOnce(&[u8], &[u8]) -> usize,
    {
        {
            let mut writer = AuWriter::new(&mut self.buf, &mut self.string_intern);
            f(&mut writer);
            if writer.msg_buf_tellp() != 0 {
                writer.term();
            }
        }
        if self.buf.tellp() != 0 {
            self.finalize_and_write(write)
        } else {
            0
        }
    }

    pub fn clear_dictionary(&mut self, clear_usage_tracker: bool) {
        self.string_intern.clear(clear_usage_tracker);
        self.emit_dict_clear();
    }

    pub fn purge_dictionary(&mut self, threshold: usize) {
        self.string_intern.purge(threshold);
    }

    pub fn reindex_dictionary(&mut self, threshold: usize) {
        self.string_intern.reindex(threshold);
        self.emit_dict_clear();
    }

    fn emit_dict_clear(&mut self) {
        self.last_dict_size = 0;
        let sor = self.dict_buf.tellp();
        {
            let mut af = AuWriter::new(&mut self.dict_buf, &mut self.string_intern);
            af.raw(b'C');
            af.value_u32(AU_FORMAT_VERSION);
            af.term();
        }
        self.backref = self.dict_buf.tellp() - sor;
    }
}

impl Default for AuEncoder {
    fn default() -> Self {
        Self::new()
    }
}

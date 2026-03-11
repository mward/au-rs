use crate::byte_source::BufferByteSource;
use crate::decoder::parse_value;
use crate::dictionary::Dictionary;
use crate::handler::{RecordHandler, ValueHandler};

pub struct AuRecordHandler<'a, V: ValueHandler> {
    dictionary: &'a mut Dictionary,
    value_handler: &'a mut V,
    str_buf: Vec<u8>,
    sor: usize,
    adding_to_dict: bool,
}

impl<'a, V: ValueHandler> AuRecordHandler<'a, V> {
    pub fn new(dictionary: &'a mut Dictionary, value_handler: &'a mut V) -> Self {
        AuRecordHandler {
            dictionary,
            value_handler,
            str_buf: Vec::with_capacity(1 << 16),
            sor: 0,
            adding_to_dict: false,
        }
    }
}

impl<V: ValueHandler> RecordHandler for AuRecordHandler<'_, V> {
    fn on_record_start(&mut self, pos: usize) {
        self.sor = pos;
    }

    fn on_header(&mut self, _version: u64, _metadata: &str) {}

    fn on_dict_clear(&mut self) {
        self.dictionary.clear(self.sor);
    }

    fn on_dict_add_start(&mut self, rel_dict_pos: usize) {
        let sor = self.sor;
        if let Ok(dict) = self.dictionary.find_dictionary(sor, rel_dict_pos) {
            self.adding_to_dict = !dict.includes(sor);
        } else {
            self.adding_to_dict = false;
        }
    }

    fn on_value(&mut self, rel_dict_pos: usize, len: usize, source: &mut BufferByteSource) {
        if let Ok(dict) = self.dictionary.find_dictionary_ref(self.sor, rel_dict_pos) {
            let mut wrapper = DictValueHandler {
                inner: self.value_handler,
                dict,
            };
            let _ = parse_value(source, &mut wrapper, 0);
        } else {
            let _ = source.skip(len);
        }
    }

    fn on_string_start(&mut self, _sov: usize, len: usize) {
        self.str_buf.clear();
        self.str_buf.reserve(len);
    }

    fn on_string_end(&mut self) {
        if self.adding_to_dict {
            let s = String::from_utf8_lossy(&self.str_buf).into_owned();
            let sor = self.sor;
            if let Some(dict) = self.dictionary.latest() {
                dict.add(sor, &s);
            }
        }
    }

    fn on_string_fragment(&mut self, fragment: &[u8]) {
        self.str_buf.extend_from_slice(fragment);
    }
}

/// Wrapper that resolves dict refs for the inner handler
struct DictValueHandler<'a, V: ValueHandler> {
    inner: &'a mut V,
    dict: &'a crate::dictionary::Dict,
}

impl<V: ValueHandler> ValueHandler for DictValueHandler<'_, V> {
    fn on_object_start(&mut self) { self.inner.on_object_start(); }
    fn on_object_end(&mut self) { self.inner.on_object_end(); }
    fn on_array_start(&mut self) { self.inner.on_array_start(); }
    fn on_array_end(&mut self) { self.inner.on_array_end(); }
    fn on_null(&mut self, pos: usize) { self.inner.on_null(pos); }
    fn on_bool(&mut self, pos: usize, val: bool) { self.inner.on_bool(pos, val); }
    fn on_int(&mut self, pos: usize, val: i64) { self.inner.on_int(pos, val); }
    fn on_uint(&mut self, pos: usize, val: u64) { self.inner.on_uint(pos, val); }
    fn on_double(&mut self, pos: usize, val: f64) { self.inner.on_double(pos, val); }
    fn on_time(&mut self, pos: usize, nanos: u64) { self.inner.on_time(pos, nanos); }

    fn on_dict_ref(&mut self, pos: usize, dict_idx: usize) {
        if let Ok(s) = self.dict.at(dict_idx) {
            self.inner.on_string_start(pos, s.len());
            self.inner.on_string_fragment(s.as_bytes());
            self.inner.on_string_end();
        }
    }

    fn on_string_start(&mut self, sov: usize, length: usize) {
        self.inner.on_string_start(sov, length);
    }
    fn on_string_end(&mut self) { self.inner.on_string_end(); }
    fn on_string_fragment(&mut self, fragment: &[u8]) {
        self.inner.on_string_fragment(fragment);
    }
}

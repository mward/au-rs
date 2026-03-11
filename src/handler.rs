#[allow(unused_variables)]
pub trait ValueHandler {
    fn on_object_start(&mut self) {}
    fn on_object_end(&mut self) {}
    fn on_array_start(&mut self) {}
    fn on_array_end(&mut self) {}
    fn on_null(&mut self, pos: usize) {}
    fn on_bool(&mut self, pos: usize, val: bool) {}
    fn on_int(&mut self, pos: usize, val: i64) {}
    fn on_uint(&mut self, pos: usize, val: u64) {}
    fn on_double(&mut self, pos: usize, val: f64) {}
    fn on_time(&mut self, pos: usize, nanos: u64) {}
    fn on_dict_ref(&mut self, pos: usize, dict_idx: usize) {}
    fn on_string_start(&mut self, sov: usize, length: usize) {}
    fn on_string_end(&mut self) {}
    fn on_string_fragment(&mut self, fragment: &[u8]) {}
}

#[allow(unused_variables)]
pub trait RecordHandler {
    fn on_record_start(&mut self, abs_pos: usize) {}
    fn on_value(&mut self, rel_dict_pos: usize, len: usize, source: &mut crate::byte_source::BufferByteSource) {
        let _ = source.skip(len);
    }
    fn on_header(&mut self, version: u64, metadata: &str) {}
    fn on_dict_clear(&mut self) {}
    fn on_dict_add_start(&mut self, rel_dict_pos: usize) {}
    fn on_string_start(&mut self, sov: usize, str_len: usize) {}
    fn on_string_end(&mut self) {}
    fn on_string_fragment(&mut self, fragment: &[u8]) {}
}

pub const AU_FORMAT_VERSION: u32 = 1;
pub const MAX_METADATA_SIZE: usize = 16 * 1024;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marker {
    Null = 0x00,
    True = 0x01,
    False = 0x02,
    Double = 0x03,
    Timestamp = 0x04,
    String = 0x05,
    Varint = 0x06,
    NegVarint = 0x07,
    PosInt64 = 0x08,
    NegInt64 = 0x09,
    DictRef = 0x0a,
    ArrayStart = 0x0b,
    ArrayEnd = 0x0c,
    ObjectStart = 0x0d,
    ObjectEnd = 0x0e,
    RecordEnd = 0x0f,
}

pub const SMALL_INT_POSITIVE: u8 = 0x60;
pub const SMALL_INT_NEGATIVE: u8 = 0x40;

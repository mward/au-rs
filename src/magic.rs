//! au file/header detection (ported from the C++ `AuMagic.h`).
//!
//! Deciding whether an input *is* an au file and deciding whether this build
//! can *decode* its version are separate questions: an au file of an
//! unsupported version should still be detected here (and then rejected by the
//! decoder's version check, which can say so specifically), rather than being
//! misdetected as, say, JSON.

use crate::byte_source::BufferByteSource;
use crate::common::{Marker, SMALL_INT_POSITIVE};

/// The magic bytes at the start of an au header record.
pub const AU_MAGIC: &[u8] = b"HAU";

/// Magic bytes plus the first byte of the encoded format version.
pub const AU_MAGIC_PREFIX_LEN: usize = AU_MAGIC.len() + 1;

/// The version is written with the writer's value encoding, so it's either a
/// "small int" (versions 0-31) or a varint marker followed by a varint. Mirrors
/// the encodings accepted by the decoder's `parse_format_version`.
#[must_use]
const fn looks_like_format_version(c: u8) -> bool {
    (c & !0x1f) == SMALL_INT_POSITIVE || c == Marker::Varint as u8
}

/// Whether `buf` starts with the au magic followed by a plausible version byte.
///
/// Accepts *any* format version, including ones this build can't read.
#[must_use]
pub fn looks_like_au_header(buf: &[u8]) -> bool {
    buf.len() >= AU_MAGIC_PREFIX_LEN
        && buf.starts_with(AU_MAGIC)
        && looks_like_format_version(buf[AU_MAGIC.len()])
}

/// Whether the source begins with an au header record. Does not consume input:
/// it inspects the not-yet-read bytes, leaving the source position unchanged.
#[must_use]
pub fn is_au_file(source: &BufferByteSource) -> bool {
    looks_like_au_header(source.remaining())
}

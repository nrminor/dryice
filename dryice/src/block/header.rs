//! Block header and layout metadata.

use crate::codec::NameEncoding;

/// A byte range within a serialized block, identified by offset and length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ByteRange {
    /// Byte offset from the start of the block payload area.
    pub offset: u64,
    /// Length in bytes.
    pub len: u64,
}

/// Header for a single block in a `dryice` file.
///
/// Contains both the semantic metadata (codec tags, record count) and
/// the layout metadata (byte ranges for each section within the block).
/// This is a private type — users interact with blocks through the
/// reader and writer APIs.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct BlockHeader {
    /// Number of records in this block.
    pub record_count: u32,

    /// Stable type tag identifying the sequence codec.
    pub sequence_codec_tag: [u8; 16],

    /// Stable type tag identifying the quality codec.
    pub quality_codec_tag: [u8; 16],

    /// How names are encoded in this block.
    pub name_encoding: NameEncoding,

    /// Width in bytes of the record-key section entries, or zero if absent.
    pub record_key_width: u16,

    /// Stable type tag identifying the record-key type, or all zeros if absent.
    pub record_key_tag: [u8; 16],

    /// Byte range of the record index section.
    pub index: ByteRange,

    /// Byte range of the names section, if present.
    pub names: Option<ByteRange>,

    /// Byte range of the sequences section.
    pub sequences: ByteRange,

    /// Byte range of the qualities section, if present.
    pub qualities: Option<ByteRange>,

    /// Byte range of the record-key section, if present.
    pub record_keys: Option<ByteRange>,
}

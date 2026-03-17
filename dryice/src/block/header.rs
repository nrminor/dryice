//! Block header and layout metadata.

use crate::codec::{NameEncoding, QualityEncoding, SequenceEncoding, SortKeyKind};

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
/// Contains both the semantic metadata (encodings, record count) and
/// the layout metadata (byte ranges for each section within the block).
/// This is a private type — users interact with blocks through the
/// reader and writer APIs.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct BlockHeader {
    /// Number of records in this block.
    pub record_count: u32,

    /// How sequences are encoded in this block.
    pub sequence_encoding: SequenceEncoding,

    /// How quality scores are encoded in this block.
    pub quality_encoding: QualityEncoding,

    /// How names are encoded in this block.
    pub name_encoding: NameEncoding,

    /// Optional sort key kind stored as an accelerator section.
    pub sort_key_kind: Option<SortKeyKind>,

    /// Byte range of the record index section.
    pub index: ByteRange,

    /// Byte range of the names section, if present.
    pub names: Option<ByteRange>,

    /// Byte range of the sequences section.
    pub sequences: ByteRange,

    /// Byte range of the qualities section, if present.
    pub qualities: Option<ByteRange>,

    /// Byte range of the sort-key section, if present.
    pub sort_keys: Option<ByteRange>,
}

//! Per-record index entries within a block.

/// A fixed-width index entry for one record within a block.
///
/// Each entry stores byte offsets and lengths into the block's
/// payload sections, allowing constant-time access to any record's
/// fields without scanning the payload data.
///
/// When a section is omitted for the entire block (for example,
/// names or qualities), the corresponding offset and length fields
/// in the index entry are ignored. Section presence is determined
/// by the block header, not by the index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RecordIndexEntry {
    /// Byte offset of this record's name within the names section.
    pub name_offset: u32,
    /// Byte length of this record's name.
    pub name_len: u32,

    /// Byte offset of this record's sequence within the sequences section.
    pub sequence_offset: u32,
    /// Byte length of this record's sequence.
    pub sequence_len: u32,

    /// Byte offset of this record's quality within the qualities section.
    pub quality_offset: u32,
    /// Byte length of this record's quality.
    pub quality_len: u32,
}

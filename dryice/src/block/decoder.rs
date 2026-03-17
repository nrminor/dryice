//! Block decoding and record extraction.

use crate::error::DryIceError;
use crate::record::SeqRecord;

use super::header::BlockHeader;
use super::index::RecordIndexEntry;

/// Decodes records from a single parsed block.
///
/// Holds the block header, parsed index, and raw section bytes.
/// Records are extracted one at a time by index position.
pub(crate) struct BlockDecoder {
    /// The parsed block header.
    #[allow(dead_code)]
    header: BlockHeader,

    /// Parsed record index entries.
    #[allow(dead_code)]
    index: Vec<RecordIndexEntry>,

    /// Raw name section bytes, if present.
    #[allow(dead_code)]
    name_bytes: Option<Vec<u8>>,

    /// Raw sequence section bytes.
    #[allow(dead_code)]
    sequence_bytes: Vec<u8>,

    /// Raw quality section bytes, if present.
    #[allow(dead_code)]
    quality_bytes: Option<Vec<u8>>,

    /// Raw sort-key section bytes, if present.
    #[allow(dead_code)]
    sort_key_bytes: Option<Vec<u8>>,

    /// Index of the next record to yield.
    #[allow(dead_code)]
    cursor: usize,
}

impl BlockDecoder {
    /// Parse a block from raw bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the block header, index, or section layout
    /// is corrupt or inconsistent.
    #[allow(dead_code)]
    pub fn from_bytes(_data: &[u8]) -> Result<Self, DryIceError> {
        todo!()
    }

    /// Extract the next record from this block, advancing the cursor.
    ///
    /// Returns `None` when all records in the block have been yielded.
    ///
    /// # Errors
    ///
    /// Returns an error if the record cannot be decoded.
    pub fn next_record(&mut self) -> Result<Option<SeqRecord>, DryIceError> {
        todo!()
    }

    /// Whether all records in this block have been yielded.
    pub fn is_exhausted(&self) -> bool {
        self.cursor >= self.index.len()
    }
}

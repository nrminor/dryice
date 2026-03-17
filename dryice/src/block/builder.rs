//! Block assembly from incoming records.

use crate::codec::{NameEncoding, QualityEncoding, SequenceEncoding, SortKeyKind};
use crate::error::DryIceError;
use crate::record::SeqRecordLike;

use super::index::RecordIndexEntry;

/// Accumulates records into a single block's worth of data.
///
/// The builder owns the block-local buffers for each payload stream
/// and the record index. When the block reaches its configured size
/// threshold, the caller should finalize it and start a new builder.
#[allow(dead_code)]
pub(crate) struct BlockBuilder {
    /// Per-record index entries accumulated so far.
    index: Vec<RecordIndexEntry>,

    /// Concatenated name bytes for all records in this block.
    name_bytes: Vec<u8>,

    /// Concatenated sequence bytes for all records in this block.
    sequence_bytes: Vec<u8>,

    /// Concatenated quality bytes for all records in this block.
    quality_bytes: Vec<u8>,

    /// Concatenated sort-key bytes, if an accelerator is configured.
    sort_key_bytes: Option<Vec<u8>>,

    /// The sequence encoding for this block.
    sequence_encoding: SequenceEncoding,

    /// The quality encoding for this block.
    quality_encoding: QualityEncoding,

    /// The name encoding for this block.
    name_encoding: NameEncoding,

    /// The sort key kind, if configured.
    sort_key_kind: Option<SortKeyKind>,

    /// Maximum number of records before the block should be flushed.
    target_records: usize,
}

impl BlockBuilder {
    /// Create a new block builder with the given encoding and sizing options.
    pub fn new(
        sequence_encoding: SequenceEncoding,
        quality_encoding: QualityEncoding,
        name_encoding: NameEncoding,
        sort_key_kind: Option<SortKeyKind>,
        target_records: usize,
    ) -> Self {
        Self {
            index: Vec::with_capacity(target_records),
            name_bytes: Vec::new(),
            sequence_bytes: Vec::new(),
            quality_bytes: Vec::new(),
            sort_key_bytes: sort_key_kind.map(|_| Vec::new()),
            sequence_encoding,
            quality_encoding,
            name_encoding,
            sort_key_kind,
            target_records,
        }
    }

    /// Append a record's data into the block-local buffers.
    ///
    /// # Errors
    ///
    /// Returns an error if the record fails validation (for example,
    /// mismatched sequence and quality lengths).
    pub fn push_record<R: SeqRecordLike>(&mut self, record: &R) -> Result<(), DryIceError> {
        let name = record.name();
        let sequence = record.sequence();
        let quality = record.quality();

        if sequence.len() != quality.len() {
            return Err(DryIceError::MismatchedSequenceAndQualityLengths {
                sequence_len: sequence.len(),
                quality_len: quality.len(),
            });
        }

        let name_offset =
            u32::try_from(self.name_bytes.len()).map_err(|_| DryIceError::CorruptBlockLayout {
                message: "name section exceeds u32 range".to_string(),
            })?;
        let sequence_offset = u32::try_from(self.sequence_bytes.len()).map_err(|_| {
            DryIceError::CorruptBlockLayout {
                message: "sequence section exceeds u32 range".to_string(),
            }
        })?;
        let quality_offset = u32::try_from(self.quality_bytes.len()).map_err(|_| {
            DryIceError::CorruptBlockLayout {
                message: "quality section exceeds u32 range".to_string(),
            }
        })?;

        let name_len = u32::try_from(name.len()).map_err(|_| DryIceError::CorruptBlockLayout {
            message: "name length exceeds u32 range".to_string(),
        })?;
        let sequence_len =
            u32::try_from(sequence.len()).map_err(|_| DryIceError::CorruptBlockLayout {
                message: "sequence length exceeds u32 range".to_string(),
            })?;
        let quality_len =
            u32::try_from(quality.len()).map_err(|_| DryIceError::CorruptBlockLayout {
                message: "quality length exceeds u32 range".to_string(),
            })?;

        self.name_bytes.extend_from_slice(name);
        self.sequence_bytes.extend_from_slice(sequence);
        self.quality_bytes.extend_from_slice(quality);

        self.index.push(RecordIndexEntry {
            name_offset,
            name_len,
            sequence_offset,
            sequence_len,
            quality_offset,
            quality_len,
        });

        Ok(())
    }

    /// Whether the block has reached its target record count.
    pub fn should_flush(&self) -> bool {
        self.index.len() >= self.target_records
    }

    /// Whether the block contains no records.
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// The number of records currently in the block.
    #[allow(dead_code)]
    pub fn record_count(&self) -> usize {
        self.index.len()
    }

    /// Finalize the block and return the encoded bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if encoding or serialization fails.
    pub fn finish_block(&mut self) -> Result<Vec<u8>, DryIceError> {
        // TODO: serialize block header, index, and payload sections
        // into a contiguous byte buffer. Reset internal state for reuse.
        todo!()
    }
}

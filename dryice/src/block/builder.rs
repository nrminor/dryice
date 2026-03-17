//! Block assembly from incoming records.

use crate::{
    block::header::{BlockHeader, ByteRange},
    codec::{NameEncoding, QualityEncoding, SequenceEncoding, SortKeyKind},
    error::DryIceError,
    format,
    record::SeqRecordLike,
};

use super::index::RecordIndexEntry;

/// Size of a serialized [`RecordIndexEntry`] in bytes (6 × u32).
const INDEX_ENTRY_SIZE: u64 = 24;

/// Accumulates records into a single block's worth of data.
///
/// The builder owns the block-local buffers for each payload stream
/// and the record index. When the block reaches its configured size
/// threshold, the caller should finalize it and start a new builder.
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
    #[allow(dead_code)]
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

    /// Finalize the block, writing the block header and payload to the
    /// given writer. Resets internal state for reuse.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or writing fails.
    pub fn write_block<W: std::io::Write>(&mut self, writer: &mut W) -> Result<(), DryIceError> {
        let record_count =
            u32::try_from(self.index.len()).map_err(|_| DryIceError::CorruptBlockLayout {
                message: "record count exceeds u32 range".to_string(),
            })?;

        let index_len = self.index.len() as u64 * INDEX_ENTRY_SIZE;
        let name_len = self.name_bytes.len() as u64;
        let seq_len = self.sequence_bytes.len() as u64;
        let qual_len = self.quality_bytes.len() as u64;

        // Compute section offsets (relative to start of payload area,
        // which begins immediately after the block header).
        let index_offset: u64 = 0;
        let names_offset = index_offset + index_len;
        let sequences_offset = names_offset + name_len;
        let qualities_offset = sequences_offset + seq_len;

        let has_names = self.name_encoding != NameEncoding::Omitted;
        let has_qualities = self.quality_encoding != QualityEncoding::Omitted;

        let header = BlockHeader {
            record_count,
            sequence_encoding: self.sequence_encoding,
            quality_encoding: self.quality_encoding,
            name_encoding: self.name_encoding,
            sort_key_kind: self.sort_key_kind,
            index: ByteRange {
                offset: index_offset,
                len: index_len,
            },
            names: if has_names {
                Some(ByteRange {
                    offset: names_offset,
                    len: name_len,
                })
            } else {
                None
            },
            sequences: ByteRange {
                offset: sequences_offset,
                len: seq_len,
            },
            qualities: if has_qualities {
                Some(ByteRange {
                    offset: qualities_offset,
                    len: qual_len,
                })
            } else {
                None
            },
            sort_keys: None, // TODO: accelerator support
        };

        // Write block header.
        format::write_block_header(writer, &header)?;

        // Write index entries.
        for entry in &self.index {
            let mut buf = [0u8; 24];
            buf[0..4].copy_from_slice(&entry.name_offset.to_le_bytes());
            buf[4..8].copy_from_slice(&entry.name_len.to_le_bytes());
            buf[8..12].copy_from_slice(&entry.sequence_offset.to_le_bytes());
            buf[12..16].copy_from_slice(&entry.sequence_len.to_le_bytes());
            buf[16..20].copy_from_slice(&entry.quality_offset.to_le_bytes());
            buf[20..24].copy_from_slice(&entry.quality_len.to_le_bytes());
            writer.write_all(&buf)?;
        }

        // Write payload sections.
        writer.write_all(&self.name_bytes)?;
        writer.write_all(&self.sequence_bytes)?;
        writer.write_all(&self.quality_bytes)?;

        // Reset for next block.
        self.index.clear();
        self.name_bytes.clear();
        self.sequence_bytes.clear();
        self.quality_bytes.clear();

        Ok(())
    }
}

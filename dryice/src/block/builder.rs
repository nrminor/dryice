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

/// Try to narrow a `usize` into a `u32`, returning a
/// [`DryIceError::SectionOverflow`] on failure.
fn to_u32(value: usize, field: &'static str) -> Result<u32, DryIceError> {
    u32::try_from(value).map_err(|_| DryIceError::SectionOverflow { field })
}

/// Configuration needed to construct a [`BlockBuilder`].
pub(crate) struct BlockBuilderConfig {
    pub sequence_encoding: SequenceEncoding,
    pub quality_encoding: QualityEncoding,
    pub name_encoding: NameEncoding,
    pub sort_key_kind: Option<SortKeyKind>,
    pub target_records: usize,
}

/// Accumulates records into a single block's worth of data.
///
/// The builder owns the block-local buffers for each payload stream
/// and the record index. When the block reaches its configured size
/// threshold, the caller should finalize it and start a new builder.
pub(crate) struct BlockBuilder {
    index: Vec<RecordIndexEntry>,
    name_bytes: Vec<u8>,
    sequence_bytes: Vec<u8>,
    quality_bytes: Vec<u8>,
    sequence_encoding: SequenceEncoding,
    quality_encoding: QualityEncoding,
    name_encoding: NameEncoding,
    sort_key_kind: Option<SortKeyKind>,
    target_records: usize,
}

impl BlockBuilder {
    /// Create a new block builder from the given configuration.
    pub fn new(config: &BlockBuilderConfig) -> Self {
        Self {
            index: Vec::with_capacity(config.target_records),
            name_bytes: Vec::new(),
            sequence_bytes: Vec::new(),
            quality_bytes: Vec::new(),
            sequence_encoding: config.sequence_encoding,
            quality_encoding: config.quality_encoding,
            name_encoding: config.name_encoding,
            sort_key_kind: config.sort_key_kind,
            target_records: config.target_records,
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

        let name_offset = to_u32(self.name_bytes.len(), "name section offset")?;
        let sequence_offset = to_u32(self.sequence_bytes.len(), "sequence section offset")?;
        let quality_offset = to_u32(self.quality_bytes.len(), "quality section offset")?;
        let name_len = to_u32(name.len(), "name length")?;
        let sequence_len = to_u32(sequence.len(), "sequence length")?;
        let quality_len = to_u32(quality.len(), "quality length")?;

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

    /// Finalize the block, writing the block header and payload to the
    /// given writer. Resets internal state for reuse.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or writing fails.
    pub fn write_block<W: std::io::Write>(&mut self, writer: &mut W) -> Result<(), DryIceError> {
        let record_count = to_u32(self.index.len(), "record count")?;

        let index_len = u64::try_from(self.index.len()).expect("index length should fit in u64")
            * INDEX_ENTRY_SIZE;
        let name_len =
            u64::try_from(self.name_bytes.len()).expect("name section length should fit in u64");
        let seq_len = u64::try_from(self.sequence_bytes.len())
            .expect("sequence section length should fit in u64");
        let qual_len = u64::try_from(self.quality_bytes.len())
            .expect("quality section length should fit in u64");

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
            sort_keys: None,
        };

        format::write_block_header(writer, &header)?;

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

        writer.write_all(&self.name_bytes)?;
        writer.write_all(&self.sequence_bytes)?;
        writer.write_all(&self.quality_bytes)?;

        self.index.clear();
        self.name_bytes.clear();
        self.sequence_bytes.clear();
        self.quality_bytes.clear();

        Ok(())
    }
}

//! Block assembly from incoming records.

use std::marker::PhantomData;

use crate::{
    block::{
        header::{BlockHeader, ByteRange},
        name::{NameCodec, OmittedNameCodec},
        quality::{OmittedQualityCodec, QualityCodec},
        sequence::SequenceCodec,
    },
    error::DryIceError,
    format,
    key::RecordKey,
    record::SeqRecordLike,
};

use super::index::RecordIndexEntry;

/// Size of a serialized [`RecordIndexEntry`] in bytes (6 × u32).
const INDEX_ENTRY_SIZE: u64 = 24;

fn to_u32(value: usize, field: &'static str) -> Result<u32, DryIceError> {
    u32::try_from(value).map_err(|_| DryIceError::SectionOverflow { field })
}

/// Configuration needed to construct a [`BlockBuilder`].
pub(crate) struct BlockBuilderConfig {
    pub record_key_width: Option<u16>,
    pub record_key_tag: Option<[u8; 16]>,
    pub target_records: usize,
}

/// Accumulates records into a single block's worth of data.
///
/// Generic over sequence, quality, and name codec types for full
/// inlining of codec encode paths — no function pointer indirection.
pub(crate) struct BlockBuilder<S: SequenceCodec, Q: QualityCodec, N: NameCodec> {
    index: Vec<RecordIndexEntry>,
    name_bytes: Vec<u8>,
    sequence_bytes: Vec<u8>,
    quality_bytes: Vec<u8>,
    record_key_bytes: Option<Vec<u8>>,
    record_key_width: u16,
    record_key_tag: [u8; 16],
    target_records: usize,
    _codec: PhantomData<S>,
    _quality: PhantomData<Q>,
    _name: PhantomData<N>,
}

impl<S: SequenceCodec, Q: QualityCodec, N: NameCodec> BlockBuilder<S, Q, N> {
    /// Create a new block builder from the given configuration.
    pub fn new(config: &BlockBuilderConfig) -> Self {
        Self {
            index: Vec::with_capacity(config.target_records),
            name_bytes: Vec::new(),
            sequence_bytes: Vec::new(),
            quality_bytes: Vec::new(),
            record_key_bytes: config.record_key_width.map(|_| Vec::new()),
            record_key_width: config.record_key_width.unwrap_or(0),
            record_key_tag: config.record_key_tag.unwrap_or([0; 16]),
            target_records: config.target_records,
            _codec: PhantomData,
            _quality: PhantomData,
            _name: PhantomData,
        }
    }

    /// Append a record's data into the block-local buffers.
    pub fn push_record<R: SeqRecordLike>(&mut self, record: &R) -> Result<(), DryIceError> {
        self.push_record_impl(record)
    }

    /// Append a record and its accelerator key into the block-local buffers.
    pub fn push_record_with_key<R: SeqRecordLike, K: RecordKey>(
        &mut self,
        record: &R,
        key: &K,
    ) -> Result<(), DryIceError> {
        debug_assert_eq!(self.record_key_width, K::WIDTH);
        debug_assert_eq!(self.record_key_tag, K::TYPE_TAG);

        self.push_record_impl(record)?;

        let key_bytes = self
            .record_key_bytes
            .as_mut()
            .ok_or(DryIceError::MissingRecordKeySection)?;
        let start = key_bytes.len();
        let width = usize::from(K::WIDTH);
        key_bytes.resize(start + width, 0);
        key.encode_into(&mut key_bytes[start..start + width]);

        Ok(())
    }

    fn push_record_impl<R: SeqRecordLike>(&mut self, record: &R) -> Result<(), DryIceError> {
        let name = record.name();
        let raw_sequence = record.sequence();
        let quality = record.quality();

        if raw_sequence.len() != quality.len() {
            return Err(DryIceError::MismatchedSequenceAndQualityLengths {
                sequence_len: raw_sequence.len(),
                quality_len: quality.len(),
            });
        }

        let name_offset = to_u32(self.name_bytes.len(), "name section offset")?;
        let sequence_offset = to_u32(self.sequence_bytes.len(), "sequence section offset")?;
        let quality_offset = to_u32(self.quality_bytes.len(), "quality section offset")?;

        N::encode_into(name, &mut self.name_bytes)?;

        S::encode_into(raw_sequence, &mut self.sequence_bytes)?;

        Q::encode_into(quality, &mut self.quality_bytes)?;

        self.index.push(RecordIndexEntry {
            name_offset,
            name_len: to_u32(name.len(), "original name length")?,
            sequence_offset,
            sequence_len: to_u32(raw_sequence.len(), "original sequence length")?,
            quality_offset,
            quality_len: to_u32(quality.len(), "original quality length")?,
        });

        Ok(())
    }

    pub fn should_flush(&self) -> bool {
        self.index.len() >= self.target_records
    }

    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

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
        let key_len = self.record_key_bytes.as_ref().map_or(0, |bytes| {
            u64::try_from(bytes.len()).expect("record-key section length should fit in u64")
        });

        let index_offset: u64 = 0;
        let names_offset = index_offset + index_len;
        let sequences_offset = names_offset + name_len;
        let qualities_offset = sequences_offset + seq_len;
        let record_keys_offset = qualities_offset + qual_len;

        let quality_omitted = Q::TYPE_TAG == <OmittedQualityCodec as QualityCodec>::TYPE_TAG;
        let names_omitted = N::TYPE_TAG == <OmittedNameCodec as NameCodec>::TYPE_TAG;

        let header = BlockHeader {
            record_count,
            sequence_codec_tag: S::TYPE_TAG,
            quality_codec_tag: Q::TYPE_TAG,
            name_codec_tag: N::TYPE_TAG,
            record_key_width: self.record_key_width,
            record_key_tag: self.record_key_tag,
            index: ByteRange {
                offset: index_offset,
                len: index_len,
            },
            names: if names_omitted {
                None
            } else {
                Some(ByteRange {
                    offset: names_offset,
                    len: name_len,
                })
            },
            sequences: ByteRange {
                offset: sequences_offset,
                len: seq_len,
            },
            qualities: if quality_omitted {
                None
            } else {
                Some(ByteRange {
                    offset: qualities_offset,
                    len: qual_len,
                })
            },
            record_keys: self.record_key_bytes.as_ref().map(|_| ByteRange {
                offset: record_keys_offset,
                len: key_len,
            }),
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
        if let Some(bytes) = &self.record_key_bytes {
            writer.write_all(bytes)?;
        }

        self.index.clear();
        self.name_bytes.clear();
        self.sequence_bytes.clear();
        self.quality_bytes.clear();
        if let Some(bytes) = &mut self.record_key_bytes {
            bytes.clear();
        }

        Ok(())
    }
}

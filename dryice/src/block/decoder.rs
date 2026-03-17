//! Block decoding and record extraction.

use crate::{
    block::header::{BlockHeader, ByteRange},
    codec::{NameEncoding, QualityEncoding, SequenceEncoding},
    error::DryIceError,
};

use super::{index::RecordIndexEntry, sequence::decode_by_tag};

/// Size of a serialized [`RecordIndexEntry`] in bytes (6 × u32).
const INDEX_ENTRY_SIZE: usize = 24;

/// Decodes records from a single parsed block.
///
/// Holds the block header, parsed index, and raw section bytes.
/// For `RawAscii` sequences, the current record is accessed via
/// borrowed slices into block-owned buffers. For `TwoBitExact`,
/// the current record's sequence is decoded into a reusable buffer
/// on each advance.
pub(crate) struct BlockDecoder {
    header: BlockHeader,
    index: Vec<RecordIndexEntry>,
    name_bytes: Option<Vec<u8>>,
    sequence_bytes: Vec<u8>,
    quality_bytes: Option<Vec<u8>>,
    record_key_bytes: Option<Vec<u8>>,
    cursor: usize,
    started: bool,
    decoded_sequence_buf: Vec<u8>,
}

fn section_len(range: Option<ByteRange>) -> Result<usize, DryIceError> {
    let len = range.map_or(0, |r| r.len);
    usize::try_from(len).map_err(|_| DryIceError::CorruptBlockLayout {
        message: "section length exceeds usize range",
    })
}

impl BlockDecoder {
    /// Parse a block's payload from the reader, given an already-parsed block header.
    pub fn from_header_and_reader<R: std::io::Read>(
        header: BlockHeader,
        reader: &mut R,
    ) -> Result<Self, DryIceError> {
        let record_count = header.record_count as usize;

        let index_byte_len = record_count * INDEX_ENTRY_SIZE;
        let mut index_buf = vec![0u8; index_byte_len];
        reader.read_exact(&mut index_buf)?;

        let mut index = Vec::with_capacity(record_count);
        for i in 0..record_count {
            let base = i * INDEX_ENTRY_SIZE;
            let b = &index_buf[base..base + INDEX_ENTRY_SIZE];
            index.push(RecordIndexEntry {
                name_offset: u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                name_len: u32::from_le_bytes([b[4], b[5], b[6], b[7]]),
                sequence_offset: u32::from_le_bytes([b[8], b[9], b[10], b[11]]),
                sequence_len: u32::from_le_bytes([b[12], b[13], b[14], b[15]]),
                quality_offset: u32::from_le_bytes([b[16], b[17], b[18], b[19]]),
                quality_len: u32::from_le_bytes([b[20], b[21], b[22], b[23]]),
            });
        }

        let name_bytes = if header.name_encoding == NameEncoding::Omitted {
            None
        } else {
            let len = section_len(header.names)?;
            let mut buf = vec![0u8; len];
            reader.read_exact(&mut buf)?;
            Some(buf)
        };

        let seq_len =
            usize::try_from(header.sequences.len).map_err(|_| DryIceError::CorruptBlockLayout {
                message: "sequence section length exceeds usize range",
            })?;
        let mut sequence_bytes = vec![0u8; seq_len];
        reader.read_exact(&mut sequence_bytes)?;

        let quality_bytes = if header.quality_encoding == QualityEncoding::Omitted {
            None
        } else {
            let len = section_len(header.qualities)?;
            let mut buf = vec![0u8; len];
            reader.read_exact(&mut buf)?;
            Some(buf)
        };

        let record_key_bytes = if header.record_keys.is_some() {
            let len = section_len(header.record_keys)?;
            let mut buf = vec![0u8; len];
            reader.read_exact(&mut buf)?;
            Some(buf)
        } else {
            None
        };

        Ok(Self {
            header,
            index,
            name_bytes,
            sequence_bytes,
            quality_bytes,
            record_key_bytes,
            cursor: 0,
            started: false,
            decoded_sequence_buf: Vec::new(),
        })
    }

    /// Advance to the next record in this block.
    ///
    /// For `TwoBitExact` blocks, this eagerly decodes the next record's
    /// sequence into a reusable internal buffer.
    pub fn advance(&mut self) -> Result<bool, DryIceError> {
        if self.started {
            self.cursor += 1;
        } else {
            self.started = true;
        }

        if self.cursor >= self.index.len() {
            return Ok(false);
        }

        if self.header.sequence_encoding != SequenceEncoding::RawAscii {
            self.decode_current_sequence()?;
        }

        Ok(true)
    }

    fn decode_current_sequence(&mut self) -> Result<(), DryIceError> {
        let entry = &self.index[self.cursor];
        let start = usize::try_from(entry.sequence_offset).expect("u32 fits in usize");
        let len = usize::try_from(entry.sequence_len).expect("u32 fits in usize");
        let encoded = &self.sequence_bytes[start..start + len];

        let original_len = usize::try_from(entry.quality_len).expect("u32 fits in usize");

        self.decoded_sequence_buf =
            decode_by_tag(self.header.sequence_encoding, encoded, original_len)?;
        Ok(())
    }

    /// The current record's name.
    pub fn current_name(&self) -> &[u8] {
        let entry = &self.index[self.cursor];
        if let Some(names) = &self.name_bytes {
            let start = usize::try_from(entry.name_offset).expect("u32 fits in usize");
            let len = usize::try_from(entry.name_len).expect("u32 fits in usize");
            &names[start..start + len]
        } else {
            &[]
        }
    }

    /// The current record's sequence.
    ///
    /// For `RawAscii` blocks, returns a borrowed slice into block-owned
    /// buffers. For `TwoBitExact` blocks, returns a slice into the
    /// decoded sequence buffer (populated during `advance()`).
    pub fn current_sequence(&self) -> &[u8] {
        if self.header.sequence_encoding == SequenceEncoding::RawAscii {
            let entry = &self.index[self.cursor];
            let start = usize::try_from(entry.sequence_offset).expect("u32 fits in usize");
            let len = usize::try_from(entry.sequence_len).expect("u32 fits in usize");
            &self.sequence_bytes[start..start + len]
        } else {
            &self.decoded_sequence_buf
        }
    }

    /// The current record's quality.
    pub fn current_quality(&self) -> &[u8] {
        let entry = &self.index[self.cursor];
        if let Some(quals) = &self.quality_bytes {
            let start = usize::try_from(entry.quality_offset).expect("u32 fits in usize");
            let len = usize::try_from(entry.quality_len).expect("u32 fits in usize");
            &quals[start..start + len]
        } else {
            &[]
        }
    }

    /// Verify that the block's record-key metadata matches the configured key type.
    pub fn verify_record_key<K: crate::key::RecordKey>(&self) -> Result<(), DryIceError> {
        if self.header.record_keys.is_none() {
            return Err(DryIceError::MissingRecordKeySection);
        }
        if self.header.record_key_width != K::WIDTH || self.header.record_key_tag != K::TYPE_TAG {
            return Err(DryIceError::RecordKeyTypeMismatch);
        }
        Ok(())
    }

    /// The current record's encoded key bytes.
    pub fn current_record_key_bytes(&self) -> Result<&[u8], DryIceError> {
        let key_bytes = self
            .record_key_bytes
            .as_ref()
            .ok_or(DryIceError::MissingRecordKeySection)?;
        let width = usize::from(self.header.record_key_width);
        let start = self.cursor * width;
        let end = start + width;
        key_bytes
            .get(start..end)
            .ok_or(DryIceError::CorruptRecordIndex {
                entry: self.cursor,
                message: "record-key bytes out of range",
            })
    }
}

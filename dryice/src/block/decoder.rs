//! Block decoding and record extraction.

use crate::{
    NameCodec, QualityCodec, SequenceCodec,
    block::header::{BlockHeader, ByteRange},
    error::DryIceError,
    fields::SelectionPlan,
};

use super::index::RecordIndexEntry;

/// Size of a serialized [`RecordIndexEntry`] in bytes (6 × u32).
const INDEX_ENTRY_SIZE: usize = 24;

/// Prepared-state for one field of the current record.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PreparedFieldState {
    #[default]
    Missing,
    Identity,
    Decoded,
}

/// Decodes records from a single parsed block.
///
/// Holds the block header, parsed index, and raw section bytes.
/// Field preparation is driven by the reader, which knows the selected
/// field set and codec types statically.
pub(crate) struct BlockDecoder {
    header: BlockHeader,
    index: Vec<RecordIndexEntry>,
    name_bytes: Option<Vec<u8>>,
    sequence_bytes: Vec<u8>,
    quality_bytes: Option<Vec<u8>>,
    record_key_bytes: Option<Vec<u8>>,
    cursor: usize,
    started: bool,
    decoded_name_buf: Vec<u8>,
    decoded_sequence_buf: Vec<u8>,
    decoded_quality_buf: Vec<u8>,
    name_state: PreparedFieldState,
    sequence_state: PreparedFieldState,
    quality_state: PreparedFieldState,
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

        let name_bytes = if header.names.is_some() {
            let len = section_len(header.names)?;
            let mut buf = vec![0u8; len];
            reader.read_exact(&mut buf)?;
            Some(buf)
        } else {
            None
        };

        let seq_len =
            usize::try_from(header.sequences.len).map_err(|_| DryIceError::CorruptBlockLayout {
                message: "sequence section length exceeds usize range",
            })?;
        let mut sequence_bytes = vec![0u8; seq_len];
        reader.read_exact(&mut sequence_bytes)?;

        let quality_bytes = if header.qualities.is_some() {
            let len = section_len(header.qualities)?;
            let mut buf = vec![0u8; len];
            reader.read_exact(&mut buf)?;
            Some(buf)
        } else {
            None
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
            decoded_name_buf: Vec::new(),
            decoded_sequence_buf: Vec::new(),
            decoded_quality_buf: Vec::new(),
            name_state: PreparedFieldState::Missing,
            sequence_state: PreparedFieldState::Missing,
            quality_state: PreparedFieldState::Missing,
        })
    }

    /// Advance to the next record in this block and prepare the fields needed by `P`.
    pub(crate) fn advance<S, Q, N, P>(&mut self) -> Result<bool, DryIceError>
    where
        S: SequenceCodec,
        Q: QualityCodec,
        N: NameCodec,
        P: SelectionPlan,
    {
        if self.started {
            self.cursor += 1;
        } else {
            self.started = true;
        }

        if self.cursor >= self.index.len() {
            self.clear_prepared_state();
            return Ok(false);
        }

        self.prepare_current_record::<S, Q, N, P>()?;
        Ok(true)
    }

    fn prepare_current_record<S, Q, N, P>(&mut self) -> Result<(), DryIceError>
    where
        S: SequenceCodec,
        Q: QualityCodec,
        N: NameCodec,
        P: SelectionPlan,
    {
        self.clear_prepared_state();

        if P::NEEDS_NAME {
            self.prepare_name::<N>()?;
        }

        if P::NEEDS_SEQUENCE {
            self.prepare_sequence::<S>()?;
        }

        if P::NEEDS_QUALITY {
            self.prepare_quality::<Q>()?;
        }

        Ok(())
    }

    fn clear_prepared_state(&mut self) {
        self.decoded_name_buf.clear();
        self.decoded_sequence_buf.clear();
        self.decoded_quality_buf.clear();
        self.name_state = PreparedFieldState::Missing;
        self.sequence_state = PreparedFieldState::Missing;
        self.quality_state = PreparedFieldState::Missing;
    }

    fn current_entry(&self) -> &RecordIndexEntry {
        &self.index[self.cursor]
    }

    fn encoded_name_bytes(&self, entry: &RecordIndexEntry) -> Result<&[u8], DryIceError> {
        let names = self
            .name_bytes
            .as_ref()
            .ok_or(DryIceError::MissingRequiredSection { section: "names" })?;
        let start = usize::try_from(entry.name_offset).expect("u32 fits in usize");
        let len = usize::try_from(entry.name_len).expect("u32 fits in usize");
        names
            .get(start..start + len)
            .ok_or(DryIceError::CorruptRecordIndex {
                entry: self.cursor,
                message: "name bytes out of range",
            })
    }

    fn encoded_sequence_bytes(&self, entry: &RecordIndexEntry) -> Result<&[u8], DryIceError> {
        let start = usize::try_from(entry.sequence_offset).expect("u32 fits in usize");
        let len = usize::try_from(entry.sequence_len).expect("u32 fits in usize");
        self.sequence_bytes
            .get(start..start + len)
            .ok_or(DryIceError::CorruptRecordIndex {
                entry: self.cursor,
                message: "sequence bytes out of range",
            })
    }

    fn encoded_quality_bytes(&self, entry: &RecordIndexEntry) -> Result<&[u8], DryIceError> {
        let qualities = self
            .quality_bytes
            .as_ref()
            .ok_or(DryIceError::MissingRequiredSection {
                section: "qualities",
            })?;
        let start = usize::try_from(entry.quality_offset).expect("u32 fits in usize");
        let len = usize::try_from(entry.quality_len).expect("u32 fits in usize");
        qualities
            .get(start..start + len)
            .ok_or(DryIceError::CorruptRecordIndex {
                entry: self.cursor,
                message: "quality bytes out of range",
            })
    }

    fn prepare_name<N>(&mut self) -> Result<(), DryIceError>
    where
        N: NameCodec,
    {
        if self.name_bytes.is_none() {
            self.name_state = PreparedFieldState::Decoded;
            return Ok(());
        }

        if N::IS_IDENTITY {
            self.name_state = PreparedFieldState::Identity;
            return Ok(());
        }

        let entry = *self.current_entry();
        let start = usize::try_from(entry.name_offset).expect("u32 fits in usize");
        let original_len = usize::try_from(entry.name_len).expect("u32 fits in usize");
        let end = start + original_len;
        let names = self
            .name_bytes
            .as_ref()
            .ok_or(DryIceError::MissingRequiredSection { section: "names" })?;
        let encoded = names
            .get(start..end)
            .ok_or(DryIceError::CorruptRecordIndex {
                entry: self.cursor,
                message: "name bytes out of range",
            })?;
        let output = &mut self.decoded_name_buf;
        output.clear();
        N::decode_to_bytes_into(encoded, original_len, output)?;
        self.name_state = PreparedFieldState::Decoded;
        Ok(())
    }

    fn prepare_sequence<S>(&mut self) -> Result<(), DryIceError>
    where
        S: SequenceCodec,
    {
        if S::IS_IDENTITY {
            self.sequence_state = PreparedFieldState::Identity;
            return Ok(());
        }

        let entry = *self.current_entry();
        let start = usize::try_from(entry.sequence_offset).expect("u32 fits in usize");
        let encoded_len = usize::try_from(entry.sequence_len).expect("u32 fits in usize");
        let original_len = usize::try_from(entry.quality_len).expect("u32 fits in usize");
        let end = start + encoded_len;
        let encoded =
            self.sequence_bytes
                .get(start..end)
                .ok_or(DryIceError::CorruptRecordIndex {
                    entry: self.cursor,
                    message: "sequence bytes out of range",
                })?;

        let output = &mut self.decoded_sequence_buf;
        output.clear();
        S::decode_into(encoded, original_len, output)?;
        self.sequence_state = PreparedFieldState::Decoded;
        Ok(())
    }

    fn prepare_quality<Q>(&mut self) -> Result<(), DryIceError>
    where
        Q: QualityCodec,
    {
        if self.quality_bytes.is_none() {
            self.quality_state = PreparedFieldState::Decoded;
            return Ok(());
        }

        if Q::IS_IDENTITY {
            self.quality_state = PreparedFieldState::Identity;
            return Ok(());
        }

        let entry = *self.current_entry();
        let start = usize::try_from(entry.quality_offset).expect("u32 fits in usize");
        let encoded_len = usize::try_from(entry.quality_len).expect("u32 fits in usize");
        let original_len = usize::try_from(entry.quality_len).expect("u32 fits in usize");
        let end = start + encoded_len;
        let qualities = self
            .quality_bytes
            .as_ref()
            .ok_or(DryIceError::MissingRequiredSection {
                section: "qualities",
            })?;
        let encoded = qualities
            .get(start..end)
            .ok_or(DryIceError::CorruptRecordIndex {
                entry: self.cursor,
                message: "quality bytes out of range",
            })?;

        let output = &mut self.decoded_quality_buf;
        output.clear();
        Q::decode_into(encoded, original_len, output)?;
        self.quality_state = PreparedFieldState::Decoded;
        Ok(())
    }

    /// The current record's prepared name bytes.
    pub fn current_name(&self) -> &[u8] {
        match self.name_state {
            PreparedFieldState::Missing => {
                panic!("name() called for a record whose name was not prepared")
            },
            PreparedFieldState::Identity => {
                if self.name_bytes.is_some() {
                    self.encoded_name_bytes(self.current_entry())
                        .expect("valid identity name slice")
                } else {
                    &[]
                }
            },
            PreparedFieldState::Decoded => &self.decoded_name_buf,
        }
    }

    /// The current record's prepared sequence bytes.
    pub fn current_sequence(&self) -> &[u8] {
        match self.sequence_state {
            PreparedFieldState::Missing => {
                panic!("sequence() called for a record whose sequence was not prepared")
            },
            PreparedFieldState::Identity => self
                .encoded_sequence_bytes(self.current_entry())
                .expect("valid identity sequence slice"),
            PreparedFieldState::Decoded => &self.decoded_sequence_buf,
        }
    }

    /// The current record's prepared quality bytes.
    pub fn current_quality(&self) -> &[u8] {
        match self.quality_state {
            PreparedFieldState::Missing => {
                panic!("quality() called for a record whose quality was not prepared")
            },
            PreparedFieldState::Identity => {
                if self.quality_bytes.is_some() {
                    self.encoded_quality_bytes(self.current_entry())
                        .expect("valid identity quality slice")
                } else {
                    &[]
                }
            },
            PreparedFieldState::Decoded => &self.decoded_quality_buf,
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

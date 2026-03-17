//! Block decoding and record extraction.

use crate::{
    codec::{NameEncoding, QualityEncoding},
    error::DryIceError,
};

use super::header::BlockHeader;
use super::index::RecordIndexEntry;

/// Size of a serialized [`RecordIndexEntry`] in bytes (6 × u32).
const INDEX_ENTRY_SIZE: usize = 24;

/// Decodes records from a single parsed block.
///
/// Holds the block header, parsed index, and raw section bytes.
/// The current record is accessed via borrowed slices into the
/// block-owned buffers — no per-record allocation occurs.
pub(crate) struct BlockDecoder {
    /// The parsed block header.
    #[allow(dead_code)]
    header: BlockHeader,

    /// Parsed record index entries.
    index: Vec<RecordIndexEntry>,

    /// Raw name section bytes, if present.
    name_bytes: Option<Vec<u8>>,

    /// Raw sequence section bytes.
    sequence_bytes: Vec<u8>,

    /// Raw quality section bytes, if present.
    quality_bytes: Option<Vec<u8>>,

    /// Index of the current record (the one most recently advanced to).
    cursor: usize,

    /// Whether the decoder has been advanced at least once.
    started: bool,
}

fn section_len(range: Option<crate::block::header::ByteRange>) -> Result<usize, DryIceError> {
    let len = range.map_or(0, |r| r.len);
    usize::try_from(len).map_err(|_| DryIceError::CorruptBlockLayout {
        message: "section length exceeds usize range".to_string(),
    })
}

impl BlockDecoder {
    /// Parse a block's payload from the reader, given an already-parsed
    /// block header.
    ///
    /// # Errors
    ///
    /// Returns an error if the payload cannot be read or is inconsistent
    /// with the header.
    pub fn from_header_and_reader<R: std::io::Read>(
        header: BlockHeader,
        reader: &mut R,
    ) -> Result<Self, DryIceError> {
        let record_count = header.record_count as usize;

        // Read index entries.
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

        // Read name section.
        let name_bytes = if header.name_encoding == NameEncoding::Omitted {
            None
        } else {
            let len = section_len(header.names)?;
            let mut buf = vec![0u8; len];
            reader.read_exact(&mut buf)?;
            Some(buf)
        };

        // Read sequence section.
        let seq_len =
            usize::try_from(header.sequences.len).map_err(|_| DryIceError::CorruptBlockLayout {
                message: "sequence section length exceeds usize range".to_string(),
            })?;
        let mut sequence_bytes = vec![0u8; seq_len];
        reader.read_exact(&mut sequence_bytes)?;

        // Read quality section.
        let quality_bytes = if header.quality_encoding == QualityEncoding::Omitted {
            None
        } else {
            let len = section_len(header.qualities)?;
            let mut buf = vec![0u8; len];
            reader.read_exact(&mut buf)?;
            Some(buf)
        };

        // TODO: read sort-key section if present.

        Ok(Self {
            header,
            index,
            name_bytes,
            sequence_bytes,
            quality_bytes,
            cursor: 0,
            started: false,
        })
    }

    /// Advance to the next record in this block.
    ///
    /// Returns `true` if a record is now current, `false` if the block
    /// is exhausted. After this returns `true`, the current record's
    /// fields can be accessed via `current_name()`, `current_sequence()`,
    /// and `current_quality()`.
    pub fn advance(&mut self) -> bool {
        if self.started {
            self.cursor += 1;
            self.cursor < self.index.len()
        } else {
            self.started = true;
            !self.index.is_empty()
        }
    }

    /// The current record's name, as a borrowed slice into block-owned
    /// buffers. Returns an empty slice if names are omitted.
    ///
    /// # Panics
    ///
    /// Panics if called before `advance()` or after the block is exhausted.
    pub fn current_name(&self) -> &[u8] {
        let entry = &self.index[self.cursor];
        if let Some(names) = &self.name_bytes {
            let start = entry.name_offset as usize;
            let end = start + entry.name_len as usize;
            &names[start..end]
        } else {
            &[]
        }
    }

    /// The current record's sequence, as a borrowed slice into
    /// block-owned buffers.
    ///
    /// # Panics
    ///
    /// Panics if called before `advance()` or after the block is exhausted.
    pub fn current_sequence(&self) -> &[u8] {
        let entry = &self.index[self.cursor];
        let start = entry.sequence_offset as usize;
        let end = start + entry.sequence_len as usize;
        &self.sequence_bytes[start..end]
    }

    /// The current record's quality, as a borrowed slice into
    /// block-owned buffers. Returns an empty slice if qualities are
    /// omitted.
    ///
    /// # Panics
    ///
    /// Panics if called before `advance()` or after the block is exhausted.
    pub fn current_quality(&self) -> &[u8] {
        let entry = &self.index[self.cursor];
        if let Some(quals) = &self.quality_bytes {
            let start = entry.quality_offset as usize;
            let end = start + entry.quality_len as usize;
            &quals[start..end]
        } else {
            &[]
        }
    }
}

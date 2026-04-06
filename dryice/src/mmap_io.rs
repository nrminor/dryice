//! Memory-mapped reader for the `dryice` format.
//!
//! This module is available behind the `mmap` feature flag and provides
//! a reader that maps a `dryice` file into memory via `memmap2`,
//! parsing blocks directly from the mapped region with no buffer
//! allocation for block loading.

use std::{fs::File, marker::PhantomData};

use memmap2::Mmap;

use crate::{
    block::{
        BlockDecoder,
        name::{NameCodec, RawNameCodec},
        quality::{QualityCodec, RawQualityCodec},
        sequence::{RawAsciiCodec, SequenceCodec},
    },
    error::DryIceError,
    format::{BLOCK_HEADER_SIZE, FILE_HEADER_SIZE, MAGIC, VERSION_MAJOR},
    key::{NoRecordKey, RecordKey},
    record::{SeqRecord, SeqRecordExt, SeqRecordLike},
};

/// A memory-mapped reader for the `dryice` format.
///
/// Maps the entire file into the process address space and parses
/// blocks directly from the mapped region. No buffer allocation
/// occurs for block loading — the OS page cache handles I/O.
///
/// The reader implements [`SeqRecordLike`] for the current record,
/// providing zero-copy access just like the sync and async readers.
pub struct MmapDryIceReader<
    S: SequenceCodec = RawAsciiCodec,
    Q: QualityCodec = RawQualityCodec,
    N: NameCodec = RawNameCodec,
    K = NoRecordKey,
> {
    mmap: Mmap,
    cursor: usize,
    current_block: Option<BlockDecoder>,
    _codec: PhantomData<S>,
    _quality: PhantomData<Q>,
    _name: PhantomData<N>,
    _key: PhantomData<K>,
}

impl MmapDryIceReader<RawAsciiCodec, RawQualityCodec, RawNameCodec, NoRecordKey> {
    /// Open a `dryice` file for memory-mapped reading with default codecs.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be mapped, or if the file
    /// header is missing, corrupt, or uses an unsupported version.
    ///
    /// # Safety
    ///
    /// Memory mapping a file that is concurrently modified by another
    /// process is undefined behavior. The caller must ensure the file
    /// is not modified while the reader exists.
    pub fn open(file: &File) -> Result<Self, DryIceError> {
        let mmap = unsafe { Mmap::map(file) }.map_err(DryIceError::Io)?;
        Self::from_mmap(mmap)
    }
}

impl<S: SequenceCodec, Q: QualityCodec, N: NameCodec, K> MmapDryIceReader<S, Q, N, K> {
    /// Open a memory-mapped reader with specific codec and key types.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be mapped, or if the file
    /// header is missing, corrupt, or uses an unsupported version.
    ///
    /// # Safety
    ///
    /// Memory mapping a file that is concurrently modified by another
    /// process is undefined behavior. The caller must ensure the file
    /// is not modified while the reader exists.
    pub fn open_with_codecs(file: &File) -> Result<Self, DryIceError> {
        let mmap = unsafe { Mmap::map(file) }.map_err(DryIceError::Io)?;
        Self::from_mmap(mmap)
    }

    fn from_mmap(mmap: Mmap) -> Result<Self, DryIceError> {
        if mmap.len() < FILE_HEADER_SIZE {
            return Err(DryIceError::InvalidMagic);
        }

        if mmap[0..4] != MAGIC {
            return Err(DryIceError::InvalidMagic);
        }

        let major = u16::from_le_bytes([mmap[4], mmap[5]]);
        if major != VERSION_MAJOR {
            return Err(DryIceError::UnsupportedFormatVersion {
                version: u32::from(major),
            });
        }

        Ok(Self {
            mmap,
            cursor: FILE_HEADER_SIZE,
            current_block: None,
            _codec: PhantomData,
            _quality: PhantomData,
            _name: PhantomData,
            _key: PhantomData,
        })
    }

    /// Advance to the next record in the file.
    ///
    /// After this returns `true`, the reader implements
    /// [`SeqRecordLike`] for the current record.
    ///
    /// # Errors
    ///
    /// Returns an error if a block header or payload is corrupt, or
    /// if codec tags don't match.
    pub fn next_record(&mut self) -> Result<bool, DryIceError> {
        if let Some(block) = &mut self.current_block
            && block.advance::<S, Q, N, crate::fields::AllFields>()?
        {
            return Ok(true);
        }

        loop {
            if self.cursor >= self.mmap.len() {
                self.current_block = None;
                return Ok(false);
            }

            if self.cursor + BLOCK_HEADER_SIZE > self.mmap.len() {
                return Err(DryIceError::CorruptBlockHeader {
                    message: "truncated block header in mapped file",
                });
            }

            let header_bytes = &self.mmap[self.cursor..self.cursor + BLOCK_HEADER_SIZE];
            let header = crate::format::read_block_header(&mut &*header_bytes)?.ok_or(
                DryIceError::CorruptBlockHeader {
                    message: "unexpected EOF parsing block header from mapped bytes",
                },
            )?;
            self.cursor += BLOCK_HEADER_SIZE;

            if header.sequence_codec_tag != S::TYPE_TAG {
                return Err(DryIceError::SequenceCodecMismatch {
                    expected: S::TYPE_TAG,
                    found: header.sequence_codec_tag,
                });
            }
            if header.quality_codec_tag != Q::TYPE_TAG {
                return Err(DryIceError::QualityCodecMismatch {
                    expected: Q::TYPE_TAG,
                    found: header.quality_codec_tag,
                });
            }
            if header.name_codec_tag != N::TYPE_TAG {
                return Err(DryIceError::NameCodecMismatch {
                    expected: N::TYPE_TAG,
                    found: header.name_codec_tag,
                });
            }

            let payload_size = Self::compute_payload_size(&header);

            debug_assert!(
                header.record_count == 0 || payload_size > 0,
                "non-empty block should have non-zero payload size"
            );

            if self.cursor + payload_size > self.mmap.len() {
                return Err(DryIceError::CorruptBlockLayout {
                    message: "block payload extends beyond mapped file",
                });
            }

            let payload = &self.mmap[self.cursor..self.cursor + payload_size];
            self.cursor += payload_size;

            let mut decoder = BlockDecoder::from_header_and_reader(header, &mut &*payload)?;
            if decoder.advance::<S, Q, N, crate::fields::AllFields>()? {
                self.current_block = Some(decoder);
                return Ok(true);
            }
        }
    }

    /// Collect all remaining records into a vector (allocates per record).
    ///
    /// # Errors
    ///
    /// Returns an error if a block cannot be parsed or decoded.
    pub fn into_records(mut self) -> Result<Vec<SeqRecord>, DryIceError> {
        let mut records = Vec::new();
        while self.next_record()? {
            records.push(self.to_seq_record()?);
        }
        Ok(records)
    }

    fn compute_payload_size(header: &crate::block::header::BlockHeader) -> usize {
        let to_usize = |v: u64| usize::try_from(v).expect("section length fits in usize");
        let mut size = to_usize(header.index.len);
        if let Some(names) = header.names {
            size += to_usize(names.len);
        }
        size += to_usize(header.sequences.len);
        if let Some(quals) = header.qualities {
            size += to_usize(quals.len);
        }
        if let Some(keys) = header.record_keys {
            size += to_usize(keys.len);
        }
        size
    }
}

impl<S: SequenceCodec, Q: QualityCodec, N: NameCodec, K: RecordKey> MmapDryIceReader<S, Q, N, K> {
    /// Decode the current record's accelerator key.
    ///
    /// # Errors
    ///
    /// Returns an error if no record key is present, if the key type
    /// doesn't match, or if decoding fails.
    pub fn record_key(&self) -> Result<K, DryIceError> {
        let block = self
            .current_block
            .as_ref()
            .ok_or(DryIceError::MissingRecordKeySection)?;
        block.verify_record_key::<K>()?;
        K::decode_from(block.current_record_key_bytes()?)
    }

    /// Advance to the next record and return only its key.
    ///
    /// # Errors
    ///
    /// Returns an error if advancing to the next record fails or if the key
    /// section is missing or incompatible with `K`.
    pub fn next_key(&mut self) -> Result<Option<K>, DryIceError> {
        if self.next_record()? {
            Ok(Some(self.record_key()?))
        } else {
            Ok(None)
        }
    }
}

impl<S: SequenceCodec, Q: QualityCodec, N: NameCodec, K> SeqRecordLike
    for MmapDryIceReader<S, Q, N, K>
{
    fn name(&self) -> &[u8] {
        debug_assert!(
            self.current_block.is_some(),
            "name() called with no current record"
        );
        self.current_block
            .as_ref()
            .map_or(&[], BlockDecoder::current_name)
    }

    fn sequence(&self) -> &[u8] {
        debug_assert!(
            self.current_block.is_some(),
            "sequence() called with no current record"
        );
        self.current_block
            .as_ref()
            .map_or(&[], BlockDecoder::current_sequence)
    }

    fn quality(&self) -> &[u8] {
        debug_assert!(
            self.current_block.is_some(),
            "quality() called with no current record"
        );
        self.current_block
            .as_ref()
            .map_or(&[], BlockDecoder::current_quality)
    }
}

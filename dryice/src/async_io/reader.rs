//! Async reader for the `dryice` format.

use std::marker::PhantomData;

use tokio::io::AsyncReadExt;

use crate::{
    block::{
        BlockDecoder,
        name::{NameCodec, RawNameCodec},
        quality::{QualityCodec, RawQualityCodec},
        sequence::{RawAsciiCodec, SequenceCodec},
    },
    error::DryIceError,
    key::{NoRecordKey, RecordKey},
    record::{SeqRecord, SeqRecordExt, SeqRecordLike},
};

use super::format as async_format;

/// Async reader for the `dryice` format.
///
/// This is the async counterpart of [`DryIceReader`](crate::DryIceReader).
/// Block loading is async; block decoding and record access are synchronous.
///
/// The reader implements [`SeqRecordLike`] for the current record,
/// providing zero-copy access to block-owned buffers just like the
/// sync reader.
pub struct AsyncDryIceReader<
    R,
    S: SequenceCodec = RawAsciiCodec,
    Q: QualityCodec = RawQualityCodec,
    N: NameCodec = RawNameCodec,
    K = NoRecordKey,
> {
    inner: R,
    current_block: Option<BlockDecoder>,
    payload_buf: Vec<u8>,
    _codec: PhantomData<S>,
    _quality: PhantomData<Q>,
    _name: PhantomData<N>,
    _key: PhantomData<K>,
}

impl<R: AsyncReadExt + Unpin>
    AsyncDryIceReader<R, RawAsciiCodec, RawQualityCodec, RawNameCodec, NoRecordKey>
{
    /// Open a `dryice` file for async reading with default codecs.
    ///
    /// # Errors
    ///
    /// Returns an error if the file header is missing, corrupt, or
    /// uses an unsupported format version.
    pub async fn new(mut inner: R) -> Result<Self, DryIceError> {
        async_format::read_file_header(&mut inner).await?;
        Ok(Self {
            inner,
            current_block: None,
            payload_buf: Vec::new(),
            _codec: PhantomData,
            _quality: PhantomData,
            _name: PhantomData,
            _key: PhantomData,
        })
    }
}

impl<R: AsyncReadExt + Unpin, S: SequenceCodec, Q: QualityCodec, N: NameCodec, K: RecordKey>
    AsyncDryIceReader<R, S, Q, N, K>
{
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
}

impl<R: AsyncReadExt + Unpin, S: SequenceCodec, Q: QualityCodec, N: NameCodec, K>
    AsyncDryIceReader<R, S, Q, N, K>
{
    /// Open a reader with specific codec and key types.
    ///
    /// # Errors
    ///
    /// Returns an error if the file header is missing, corrupt, or
    /// uses an unsupported format version.
    pub async fn with_codecs(mut inner: R) -> Result<Self, DryIceError> {
        async_format::read_file_header(&mut inner).await?;
        Ok(Self {
            inner,
            current_block: None,
            payload_buf: Vec::new(),
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
    /// Returns an error if a block cannot be loaded or decoded, or
    /// if codec tags don't match.
    pub async fn next_record(&mut self) -> Result<bool, DryIceError> {
        if let Some(block) = &mut self.current_block
            && block.advance(S::decode_into, Q::decode_into, N::decode_to_bytes_into)?
        {
            return Ok(true);
        }

        loop {
            if let Some(header) = async_format::read_block_header(&mut self.inner).await? {
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
                self.payload_buf.clear();
                self.payload_buf.resize(payload_size, 0);
                self.inner.read_exact(&mut self.payload_buf).await?;

                let mut decoder =
                    BlockDecoder::from_header_and_reader(header, &mut self.payload_buf.as_slice())?;
                if decoder.advance(S::decode_into, Q::decode_into, N::decode_to_bytes_into)? {
                    self.current_block = Some(decoder);
                    return Ok(true);
                }
            } else {
                self.current_block = None;
                return Ok(false);
            }
        }
    }

    /// Collect all remaining records into a vector (allocates per record).
    ///
    /// # Errors
    ///
    /// Returns an error if a block cannot be loaded or decoded.
    pub async fn into_records(mut self) -> Result<Vec<SeqRecord>, DryIceError> {
        let mut records = Vec::new();
        while self.next_record().await? {
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

        debug_assert!(
            header.record_count == 0 || size > 0,
            "non-empty block should have non-zero payload size"
        );

        size
    }
}

impl<R: AsyncReadExt + Unpin, S: SequenceCodec, Q: QualityCodec, N: NameCodec, K> SeqRecordLike
    for AsyncDryIceReader<R, S, Q, N, K>
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

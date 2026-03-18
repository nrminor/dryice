//! Reader for the `dryice` format.

use std::{io::Read, marker::PhantomData};

use crate::{
    block::{
        BlockDecoder,
        name::{NameCodec, RawNameCodec},
        quality::{QualityCodec, RawQualityCodec},
        sequence::{RawAsciiCodec, SequenceCodec, TwoBitExactCodec},
    },
    error::DryIceError,
    format,
    key::{Bytes8Key, Bytes16Key, NoRecordKey, RecordKey},
    record::{SeqRecord, SeqRecordExt, SeqRecordLike},
};

/// Reads sequencing records from a `dryice` file.
pub struct DryIceReader<
    R,
    S = RawAsciiCodec,
    Q = RawQualityCodec,
    N = RawNameCodec,
    K = NoRecordKey,
> {
    inner: R,
    current_block: Option<BlockDecoder>,
    _codec: PhantomData<S>,
    _quality: PhantomData<Q>,
    _name: PhantomData<N>,
    _key: PhantomData<K>,
}

impl<R: Read> DryIceReader<R, RawAsciiCodec, RawQualityCodec, RawNameCodec, NoRecordKey> {
    /// Open a `dryice` file for reading with default codecs.
    ///
    /// # Errors
    ///
    /// Returns an error if the file header is missing, corrupt, or uses an
    /// unsupported format version.
    pub fn new(mut inner: R) -> Result<Self, DryIceError> {
        format::read_file_header(&mut inner)?;
        Ok(Self {
            inner,
            current_block: None,
            _codec: PhantomData,
            _quality: PhantomData,
            _name: PhantomData,
            _key: PhantomData,
        })
    }

    /// Open a reader configured for the built-in 2-bit exact sequence codec.
    ///
    /// # Errors
    ///
    /// Returns an error if the file header is missing, corrupt, or uses an
    /// unsupported format version.
    pub fn with_two_bit_exact(
        mut inner: R,
    ) -> Result<
        DryIceReader<R, TwoBitExactCodec, RawQualityCodec, RawNameCodec, NoRecordKey>,
        DryIceError,
    > {
        format::read_file_header(&mut inner)?;
        Ok(DryIceReader {
            inner,
            current_block: None,
            _codec: PhantomData,
            _quality: PhantomData,
            _name: PhantomData,
            _key: PhantomData,
        })
    }

    /// Open a reader configured for user-defined codecs.
    ///
    /// # Errors
    ///
    /// Returns an error if the file header is missing, corrupt, or uses an
    /// unsupported format version.
    pub fn with_codecs<S: SequenceCodec, Q: QualityCodec, N: NameCodec>(
        mut inner: R,
    ) -> Result<DryIceReader<R, S, Q, N, NoRecordKey>, DryIceError> {
        format::read_file_header(&mut inner)?;
        Ok(DryIceReader {
            inner,
            current_block: None,
            _codec: PhantomData,
            _quality: PhantomData,
            _name: PhantomData,
            _key: PhantomData,
        })
    }

    /// Open a reader configured for a user-defined record-key type
    /// with default codecs.
    ///
    /// # Errors
    ///
    /// Returns an error if the file header is missing, corrupt, or uses an
    /// unsupported format version.
    pub fn with_record_key<K2: RecordKey>(
        mut inner: R,
    ) -> Result<DryIceReader<R, RawAsciiCodec, RawQualityCodec, RawNameCodec, K2>, DryIceError>
    {
        format::read_file_header(&mut inner)?;
        Ok(DryIceReader {
            inner,
            current_block: None,
            _codec: PhantomData,
            _quality: PhantomData,
            _name: PhantomData,
            _key: PhantomData,
        })
    }

    /// Open a reader configured for the built-in 8-byte key type.
    ///
    /// # Errors
    ///
    /// Returns an error if the file header is missing, corrupt, or uses an
    /// unsupported format version.
    pub fn with_bytes8_key(
        inner: R,
    ) -> Result<DryIceReader<R, RawAsciiCodec, RawQualityCodec, RawNameCodec, Bytes8Key>, DryIceError>
    {
        Self::with_record_key::<Bytes8Key>(inner)
    }

    /// Open a reader configured for the built-in 16-byte key type.
    ///
    /// # Errors
    ///
    /// Returns an error if the file header is missing, corrupt, or uses an
    /// unsupported format version.
    pub fn with_bytes16_key(
        inner: R,
    ) -> Result<
        DryIceReader<R, RawAsciiCodec, RawQualityCodec, RawNameCodec, Bytes16Key>,
        DryIceError,
    > {
        Self::with_record_key::<Bytes16Key>(inner)
    }
}

impl<R: Read, S: SequenceCodec, Q: QualityCodec, N: NameCodec, K: RecordKey>
    DryIceReader<R, S, Q, N, K>
{
    /// Decode the current record's accelerator key.
    ///
    /// # Errors
    ///
    /// Returns an error if no record key is present in the current block, if the
    /// configured key type does not match the block's key metadata, or if the key
    /// bytes cannot be decoded into `K`.
    pub fn record_key(&self) -> Result<K, DryIceError> {
        let block = self
            .current_block
            .as_ref()
            .ok_or(DryIceError::MissingRecordKeySection)?;
        block.verify_record_key::<K>()?;
        K::decode_from(block.current_record_key_bytes()?)
    }
}

impl<R: Read, S: SequenceCodec, Q: QualityCodec, N: NameCodec, K> DryIceReader<R, S, Q, N, K> {
    /// Advance to the next record in the file.
    ///
    /// # Errors
    ///
    /// Returns an error if a block header or block payload cannot be read or
    /// decoded, or if the block's codec tags do not match the reader's
    /// configured codecs.
    pub fn next_record(&mut self) -> Result<bool, DryIceError> {
        if let Some(block) = &mut self.current_block
            && block.advance(S::decode_into, Q::decode_into, N::decode_to_bytes_into)?
        {
            return Ok(true);
        }

        loop {
            if let Some(header) = format::read_block_header(&mut self.inner)? {
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

                let mut decoder = BlockDecoder::from_header_and_reader(header, &mut self.inner)?;
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

    /// Consume this reader into an iterator of owned [`SeqRecord`] values.
    pub fn into_records(self) -> DryIceRecords<R, S, Q, N, K> {
        DryIceRecords { reader: self }
    }
}

impl<R: Read, S: SequenceCodec, Q: QualityCodec, N: NameCodec, K> SeqRecordLike
    for DryIceReader<R, S, Q, N, K>
{
    fn name(&self) -> &[u8] {
        self.current_block
            .as_ref()
            .expect("name() called with no current record")
            .current_name()
    }

    fn sequence(&self) -> &[u8] {
        self.current_block
            .as_ref()
            .expect("sequence() called with no current record")
            .current_sequence()
    }

    fn quality(&self) -> &[u8] {
        self.current_block
            .as_ref()
            .expect("quality() called with no current record")
            .current_quality()
    }
}

/// Iterator over records in a `dryice` file, yielding owned [`SeqRecord`] values.
pub struct DryIceRecords<
    R,
    S = RawAsciiCodec,
    Q = RawQualityCodec,
    N = RawNameCodec,
    K = NoRecordKey,
> {
    reader: DryIceReader<R, S, Q, N, K>,
}

impl<R: Read, S: SequenceCodec, Q: QualityCodec, N: NameCodec, K> Iterator
    for DryIceRecords<R, S, Q, N, K>
{
    type Item = Result<SeqRecord, DryIceError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.reader.next_record() {
            Ok(true) => Some(self.reader.to_seq_record()),
            Ok(false) => None,
            Err(e) => Some(Err(e)),
        }
    }
}

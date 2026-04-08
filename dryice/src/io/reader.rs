//! Reader for the `dryice` format.

use std::{io::Read, marker::PhantomData};

use crate::{
    block::{
        BlockDecoder,
        name::{NameCodec, OmittedNameCodec, RawNameCodec},
        quality::{OmittedQualityCodec, QualityCodec, RawQualityCodec},
        sequence::{OmittedSequenceCodec, RawAsciiCodec, SequenceCodec, TwoBitExactCodec},
    },
    error::DryIceError,
    fields::{AllFields, HasKey, HasName, HasQuality, HasSequence, SelectionExpr, SelectionPlan},
    format,
    key::{Bytes8Key, Bytes16Key, NoRecordKey, RecordKey},
    record::{SeqRecord, SeqRecordExt, SeqRecordLike},
};

/// Private marker type used to track a missing reader source in the builder.
#[doc(hidden)]
pub struct MissingInner;

/// Builder-state marker for the default full-row read mode.
#[doc(hidden)]
pub struct ReadAllFields;

/// Builder-state marker for a future selected-read mode.
#[doc(hidden)]
pub struct ReadSelectedFields<F>(PhantomData<F>);

/// Reader type returned when a field selection is specified on the builder.
///
/// This reader still advances through whole records in order, but it only
/// prepares the fields implied by the selected field set. The selected field
/// methods then live on [`SelectedRecord`] rather than on the reader itself.
pub struct SelectedDryIceReader<
    R,
    S = RawAsciiCodec,
    Q = RawQualityCodec,
    N = RawNameCodec,
    K = NoRecordKey,
    F = ReadAllFields,
> {
    inner: DryIceReader<R, S, Q, N, K>,
    _fields: PhantomData<F>,
}

/// Borrowed current-record view returned by a selected reader.
///
/// The methods available on this view are determined by the selected field set
/// used to build the reader. If a field was not selected, no accessor for that
/// field is available on the type.
pub struct SelectedRecord<
    'a,
    R,
    S = RawAsciiCodec,
    Q = RawQualityCodec,
    N = RawNameCodec,
    K = NoRecordKey,
    F = ReadAllFields,
> {
    reader: &'a DryIceReader<R, S, Q, N, K>,
    _fields: PhantomData<F>,
}

/// Convenient alias for the borrowed current-record view returned by
/// [`SelectedDryIceReader::next_record`].
pub type SelectedRecordView<'a, R, S, Q, N, K, F> = SelectedRecord<'a, R, S, Q, N, K, F>;

/// Convenient alias for the optional next-record return value of a selected reader.
pub type SelectedNextRecord<'a, R, S, Q, N, K, F> =
    Option<SelectedRecordView<'a, R, S, Q, N, K, F>>;

fn verify_block_codecs<S, Q, N>(
    header: &crate::block::header::BlockHeader,
) -> Result<(), DryIceError>
where
    S: SequenceCodec,
    Q: QualityCodec,
    N: NameCodec,
{
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
    Ok(())
}

/// Builder for [`DryIceReader`].
pub struct DryIceReaderBuilder<
    R = MissingInner,
    S = RawAsciiCodec,
    Q = RawQualityCodec,
    N = RawNameCodec,
    K = NoRecordKey,
    M = ReadAllFields,
> {
    inner: R,
    _codec: PhantomData<S>,
    _quality: PhantomData<Q>,
    _name: PhantomData<N>,
    _key: PhantomData<K>,
    _mode: PhantomData<M>,
}

impl
    DryIceReaderBuilder<
        MissingInner,
        RawAsciiCodec,
        RawQualityCodec,
        RawNameCodec,
        NoRecordKey,
        ReadAllFields,
    >
{
    fn new() -> Self {
        Self {
            inner: MissingInner,
            _codec: PhantomData,
            _quality: PhantomData,
            _name: PhantomData,
            _key: PhantomData,
            _mode: PhantomData,
        }
    }
}

impl<S, Q, N, K, M> DryIceReaderBuilder<MissingInner, S, Q, N, K, M> {
    /// Set the reader's input source.
    #[must_use]
    pub fn inner<R>(self, inner: R) -> DryIceReaderBuilder<R, S, Q, N, K, M> {
        DryIceReaderBuilder {
            inner,
            _codec: self._codec,
            _quality: self._quality,
            _name: self._name,
            _key: self._key,
            _mode: self._mode,
        }
    }
}

impl<R, Q, N, K, M> DryIceReaderBuilder<R, RawAsciiCodec, Q, N, K, M> {
    /// Configure the reader to use a user-defined sequence codec.
    #[must_use]
    pub fn sequence_codec<S: SequenceCodec>(self) -> DryIceReaderBuilder<R, S, Q, N, K, M> {
        DryIceReaderBuilder {
            inner: self.inner,
            _codec: PhantomData,
            _quality: PhantomData,
            _name: PhantomData,
            _key: PhantomData,
            _mode: PhantomData,
        }
    }

    /// Configure the reader to use the built-in 2-bit exact codec.
    #[must_use]
    pub fn two_bit_exact(self) -> DryIceReaderBuilder<R, TwoBitExactCodec, Q, N, K, M> {
        self.sequence_codec::<TwoBitExactCodec>()
    }

    /// Configure the reader to expect omitted sequence payloads.
    #[must_use]
    pub fn omit_sequence(self) -> DryIceReaderBuilder<R, OmittedSequenceCodec, Q, N, K, M> {
        self.sequence_codec::<OmittedSequenceCodec>()
    }
}

impl<R, S, N, K, M> DryIceReaderBuilder<R, S, RawQualityCodec, N, K, M> {
    /// Configure the reader to use a user-defined quality codec.
    #[must_use]
    pub fn quality_codec<Q: QualityCodec>(self) -> DryIceReaderBuilder<R, S, Q, N, K, M> {
        DryIceReaderBuilder {
            inner: self.inner,
            _codec: PhantomData,
            _quality: PhantomData,
            _name: PhantomData,
            _key: PhantomData,
            _mode: PhantomData,
        }
    }

    /// Configure the reader to expect omitted quality payloads.
    #[must_use]
    pub fn omit_quality(self) -> DryIceReaderBuilder<R, S, OmittedQualityCodec, N, K, M> {
        self.quality_codec::<OmittedQualityCodec>()
    }
}

impl<R, S, Q, K, M> DryIceReaderBuilder<R, S, Q, RawNameCodec, K, M> {
    /// Configure the reader to use a user-defined name codec.
    #[must_use]
    pub fn name_codec<N: NameCodec>(self) -> DryIceReaderBuilder<R, S, Q, N, K, M> {
        DryIceReaderBuilder {
            inner: self.inner,
            _codec: PhantomData,
            _quality: PhantomData,
            _name: PhantomData,
            _key: PhantomData,
            _mode: PhantomData,
        }
    }

    /// Configure the reader to expect omitted name payloads.
    #[must_use]
    pub fn omit_names(self) -> DryIceReaderBuilder<R, S, Q, OmittedNameCodec, K, M> {
        self.name_codec::<OmittedNameCodec>()
    }
}

impl<R, S, Q, N, M> DryIceReaderBuilder<R, S, Q, N, NoRecordKey, M> {
    /// Configure the reader for a user-defined record-key type.
    #[must_use]
    pub fn record_key<K: RecordKey>(self) -> DryIceReaderBuilder<R, S, Q, N, K, M> {
        DryIceReaderBuilder {
            inner: self.inner,
            _codec: PhantomData,
            _quality: PhantomData,
            _name: PhantomData,
            _key: PhantomData,
            _mode: PhantomData,
        }
    }

    /// Configure the reader for the built-in 8-byte key type.
    #[must_use]
    pub fn bytes8_key(self) -> DryIceReaderBuilder<R, S, Q, N, Bytes8Key, M> {
        self.record_key::<Bytes8Key>()
    }

    /// Configure the reader for the built-in 16-byte key type.
    #[must_use]
    pub fn bytes16_key(self) -> DryIceReaderBuilder<R, S, Q, N, Bytes16Key, M> {
        self.record_key::<Bytes16Key>()
    }
}

impl<R, S, Q, N, K> DryIceReaderBuilder<R, S, Q, N, K, ReadAllFields> {
    /// Configure a selected-decoding projection for reads built from this builder.
    ///
    /// The resulting reader still reads full blocks from disk, but it will only
    /// decode the fields named in `fields` for each record it advances through.
    /// This is useful for intermediate scan-style passes that need only a subset
    /// of each record, such as sequence-only filtering or sequence-plus-key
    /// partitioning.
    #[must_use]
    pub fn select<F: SelectionExpr>(
        self,
        _fields: F,
    ) -> DryIceReaderBuilder<R, S, Q, N, K, ReadSelectedFields<F>> {
        DryIceReaderBuilder {
            inner: self.inner,
            _codec: PhantomData,
            _quality: PhantomData,
            _name: PhantomData,
            _key: PhantomData,
            _mode: PhantomData,
        }
    }
}

impl<R: Read, S: SequenceCodec, Q: QualityCodec, N: NameCodec>
    DryIceReaderBuilder<R, S, Q, N, NoRecordKey, ReadAllFields>
{
    /// Build an unkeyed reader in the default full-row mode.
    pub fn build(mut self) -> Result<DryIceReader<R, S, Q, N, NoRecordKey>, DryIceError> {
        format::read_file_header(&mut self.inner)?;
        Ok(DryIceReader {
            inner: self.inner,
            current_block: None,
            _codec: PhantomData,
            _quality: PhantomData,
            _name: PhantomData,
            _key: PhantomData,
        })
    }
}

impl<R: Read, S: SequenceCodec, Q: QualityCodec, N: NameCodec, K: RecordKey>
    DryIceReaderBuilder<R, S, Q, N, K, ReadAllFields>
{
    /// Build a keyed reader in the default full-row mode.
    pub fn build(mut self) -> Result<DryIceReader<R, S, Q, N, K>, DryIceError> {
        format::read_file_header(&mut self.inner)?;
        Ok(DryIceReader {
            inner: self.inner,
            current_block: None,
            _codec: PhantomData,
            _quality: PhantomData,
            _name: PhantomData,
            _key: PhantomData,
        })
    }
}

impl<R: Read, S: SequenceCodec, Q: QualityCodec, N: NameCodec, F: SelectionPlan>
    DryIceReaderBuilder<R, S, Q, N, NoRecordKey, ReadSelectedFields<F>>
{
    /// Build an unkeyed selected reader.
    pub fn build(
        mut self,
    ) -> Result<SelectedDryIceReader<R, S, Q, N, NoRecordKey, F>, DryIceError> {
        format::read_file_header(&mut self.inner)?;
        Ok(SelectedDryIceReader {
            inner: DryIceReader {
                inner: self.inner,
                current_block: None,
                _codec: PhantomData,
                _quality: PhantomData,
                _name: PhantomData,
                _key: PhantomData,
            },
            _fields: PhantomData,
        })
    }
}

impl<R: Read, S: SequenceCodec, Q: QualityCodec, N: NameCodec, K: RecordKey, F: SelectionPlan>
    DryIceReaderBuilder<R, S, Q, N, K, ReadSelectedFields<F>>
{
    /// Build a keyed selected reader.
    pub fn build(mut self) -> Result<SelectedDryIceReader<R, S, Q, N, K, F>, DryIceError> {
        format::read_file_header(&mut self.inner)?;
        Ok(SelectedDryIceReader {
            inner: DryIceReader {
                inner: self.inner,
                current_block: None,
                _codec: PhantomData,
                _quality: PhantomData,
                _name: PhantomData,
                _key: PhantomData,
            },
            _fields: PhantomData,
        })
    }
}

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

impl DryIceReader<MissingInner, RawAsciiCodec, RawQualityCodec, RawNameCodec, NoRecordKey> {
    /// Start building a new reader.
    #[must_use]
    pub fn builder() -> DryIceReaderBuilder<
        MissingInner,
        RawAsciiCodec,
        RawQualityCodec,
        RawNameCodec,
        NoRecordKey,
        ReadAllFields,
    > {
        DryIceReaderBuilder::new()
    }
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

impl<R: Read> DryIceReader<R> {
    /// Open a reader with fully user-specified codec and key type
    /// parameters.
    ///
    /// This is the most general constructor, intended for library
    /// authors who need to configure all four type parameters at
    /// once. Most users should prefer [`new`](Self::new),
    /// [`with_codecs`](Self::with_codecs), or the convenience
    /// constructors instead.
    ///
    /// # Errors
    ///
    /// Returns an error if the file header is missing, corrupt,
    /// or uses an unsupported format version.
    pub fn open<S: SequenceCodec, Q: QualityCodec, N: NameCodec, K: RecordKey>(
        mut inner: R,
    ) -> Result<DryIceReader<R, S, Q, N, K>, DryIceError> {
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
}

impl<R, S, Q, N, K, F> SelectedDryIceReader<R, S, Q, N, K, F>
where
    R: Read,
    S: SequenceCodec,
    Q: QualityCodec,
    N: NameCodec,
    F: SelectionPlan,
{
    /// Advance to the next selected record in the file.
    ///
    /// # Errors
    ///
    /// Returns an error if the next block header or payload cannot be read or
    /// decoded, or if the on-disk codec tags do not match the reader's
    /// configured codecs.
    pub fn next_record(&mut self) -> Result<SelectedNextRecord<'_, R, S, Q, N, K, F>, DryIceError> {
        if self.inner.next_record_prepared::<F>()? {
            Ok(Some(SelectedRecord {
                reader: &self.inner,
                _fields: PhantomData,
            }))
        } else {
            Ok(None)
        }
    }
}

impl<R, S, Q, N, K, F> SelectedRecord<'_, R, S, Q, N, K, F>
where
    R: Read,
    S: SequenceCodec,
    Q: QualityCodec,
    N: NameCodec,
    F: SelectionExpr + HasName,
{
    /// Borrow the selected record name.
    #[must_use]
    pub fn name(&self) -> &[u8] {
        self.reader.name()
    }
}

impl<R, S, Q, N, K, F> SelectedRecord<'_, R, S, Q, N, K, F>
where
    R: Read,
    S: SequenceCodec,
    Q: QualityCodec,
    N: NameCodec,
    F: SelectionExpr + HasSequence,
{
    /// Borrow the selected record sequence.
    #[must_use]
    pub fn sequence(&self) -> &[u8] {
        self.reader.sequence()
    }
}

impl<R, S, Q, N, K, F> SelectedRecord<'_, R, S, Q, N, K, F>
where
    R: Read,
    S: SequenceCodec,
    Q: QualityCodec,
    N: NameCodec,
    F: SelectionExpr + HasQuality,
{
    /// Borrow the selected record quality.
    #[must_use]
    pub fn quality(&self) -> &[u8] {
        self.reader.quality()
    }
}

impl<R, S, Q, N, K, F> SelectedRecord<'_, R, S, Q, N, K, F>
where
    R: Read,
    S: SequenceCodec,
    Q: QualityCodec,
    N: NameCodec,
    K: RecordKey,
    F: SelectionExpr + HasKey,
{
    /// Decode the selected record key.
    ///
    /// # Errors
    ///
    /// Returns an error if the current block does not contain keys of type `K`
    /// or if the key bytes cannot be decoded into `K`.
    pub fn record_key(&self) -> Result<K, DryIceError> {
        self.reader.record_key()
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

impl<R: Read, S: SequenceCodec, Q: QualityCodec, N: NameCodec, K> DryIceReader<R, S, Q, N, K> {
    fn next_record_prepared<P>(&mut self) -> Result<bool, DryIceError>
    where
        P: SelectionPlan,
    {
        if let Some(block) = &mut self.current_block
            && block.advance::<S, Q, N, P>()?
        {
            return Ok(true);
        }

        loop {
            if let Some(header) = format::read_block_header(&mut self.inner)? {
                verify_block_codecs::<S, Q, N>(&header)?;
                let mut decoder = BlockDecoder::from_header_and_reader(header, &mut self.inner)?;
                if decoder.advance::<S, Q, N, P>()? {
                    self.current_block = Some(decoder);
                    return Ok(true);
                }
            } else {
                self.current_block = None;
                return Ok(false);
            }
        }
    }

    /// Advance to the next record in the file.
    ///
    /// # Errors
    ///
    /// Returns an error if a block header or block payload cannot be read or
    /// decoded, or if the block's codec tags do not match the reader's
    /// configured codecs.
    pub fn next_record(&mut self) -> Result<bool, DryIceError> {
        self.next_record_prepared::<AllFields>()
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

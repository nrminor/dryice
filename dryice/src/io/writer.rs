//! Writer for the `dryice` format.

use std::{io::Write, marker::PhantomData};

use crate::{
    block::{
        BlockBuilder, BlockBuilderConfig,
        name::{NameCodec, OmittedNameCodec, RawNameCodec, SplitNameCodec},
        quality::{BinnedQualityCodec, OmittedQualityCodec, QualityCodec, RawQualityCodec},
        sequence::{RawAsciiCodec, SequenceCodec, TwoBitExactCodec, TwoBitLossyNCodec},
    },
    config::{BlockLayoutOptions, BlockSizePolicy, DryIceWriterOptions},
    error::DryIceError,
    format,
    key::{Bytes8Key, Bytes16Key, NoRecordKey, RecordKey},
    record::SeqRecordLike,
};

/// Private marker type used to track a missing writer target in the builder.
pub struct MissingInner;

/// Builder for [`DryIceWriter`].
pub struct DryIceWriterBuilder<
    W = MissingInner,
    S = RawAsciiCodec,
    Q = RawQualityCodec,
    N = RawNameCodec,
    K = NoRecordKey,
> {
    inner: W,
    target_block_records: usize,
    _codec: PhantomData<S>,
    _quality: PhantomData<Q>,
    _name: PhantomData<N>,
    _key: PhantomData<K>,
}

impl DryIceWriterBuilder<MissingInner, RawAsciiCodec, RawQualityCodec, RawNameCodec, NoRecordKey> {
    fn new() -> Self {
        Self {
            inner: MissingInner,
            target_block_records: 8192,
            _codec: PhantomData,
            _quality: PhantomData,
            _name: PhantomData,
            _key: PhantomData,
        }
    }
}

impl<W, S, Q, N, K> DryIceWriterBuilder<W, S, Q, N, K> {
    /// Set the block size policy in records.
    #[must_use]
    pub fn target_block_records(mut self, n: usize) -> Self {
        self.target_block_records = n;
        self
    }
}

impl<S, Q, N, K> DryIceWriterBuilder<MissingInner, S, Q, N, K> {
    /// Set the writer's output target.
    #[must_use]
    pub fn inner<W>(self, inner: W) -> DryIceWriterBuilder<W, S, Q, N, K> {
        DryIceWriterBuilder {
            inner,
            target_block_records: self.target_block_records,
            _codec: PhantomData,
            _quality: PhantomData,
            _name: PhantomData,
            _key: PhantomData,
        }
    }
}

impl<W, Q, N, K> DryIceWriterBuilder<W, RawAsciiCodec, Q, N, K> {
    /// Configure the writer to use a user-defined sequence codec.
    #[must_use]
    pub fn sequence_codec<S: SequenceCodec>(self) -> DryIceWriterBuilder<W, S, Q, N, K> {
        DryIceWriterBuilder {
            inner: self.inner,
            target_block_records: self.target_block_records,
            _codec: PhantomData,
            _quality: PhantomData,
            _name: PhantomData,
            _key: PhantomData,
        }
    }

    /// Configure the writer to use the built-in 2-bit exact codec.
    #[must_use]
    pub fn two_bit_exact(self) -> DryIceWriterBuilder<W, TwoBitExactCodec, Q, N, K> {
        self.sequence_codec::<TwoBitExactCodec>()
    }

    /// Configure the writer to use the built-in lossy 2-bit codec
    /// that collapses all ambiguous bases to N.
    #[must_use]
    pub fn two_bit_lossy_n(self) -> DryIceWriterBuilder<W, TwoBitLossyNCodec, Q, N, K> {
        self.sequence_codec::<TwoBitLossyNCodec>()
    }
}

impl<W, S, N, K> DryIceWriterBuilder<W, S, RawQualityCodec, N, K> {
    /// Configure the writer to use a user-defined quality codec.
    #[must_use]
    pub fn quality_codec<Q: QualityCodec>(self) -> DryIceWriterBuilder<W, S, Q, N, K> {
        DryIceWriterBuilder {
            inner: self.inner,
            target_block_records: self.target_block_records,
            _codec: PhantomData,
            _quality: PhantomData,
            _name: PhantomData,
            _key: PhantomData,
        }
    }

    /// Configure the writer to use the built-in binned quality codec.
    #[must_use]
    pub fn binned_quality(self) -> DryIceWriterBuilder<W, S, BinnedQualityCodec, N, K> {
        self.quality_codec::<BinnedQualityCodec>()
    }

    /// Configure the writer to omit quality scores entirely.
    #[must_use]
    pub fn omit_quality(self) -> DryIceWriterBuilder<W, S, OmittedQualityCodec, N, K> {
        self.quality_codec::<OmittedQualityCodec>()
    }
}

impl<W, S, Q, K> DryIceWriterBuilder<W, S, Q, RawNameCodec, K> {
    /// Configure the writer to use a user-defined name codec.
    #[must_use]
    pub fn name_codec<N: NameCodec>(self) -> DryIceWriterBuilder<W, S, Q, N, K> {
        DryIceWriterBuilder {
            inner: self.inner,
            target_block_records: self.target_block_records,
            _codec: PhantomData,
            _quality: PhantomData,
            _name: PhantomData,
            _key: PhantomData,
        }
    }

    /// Configure the writer to omit names entirely.
    #[must_use]
    pub fn omit_names(self) -> DryIceWriterBuilder<W, S, Q, OmittedNameCodec, K> {
        self.name_codec::<OmittedNameCodec>()
    }

    /// Configure the writer to split names on the first space.
    #[must_use]
    pub fn split_names(self) -> DryIceWriterBuilder<W, S, Q, SplitNameCodec, K> {
        self.name_codec::<SplitNameCodec>()
    }
}

impl<W, S, Q, N> DryIceWriterBuilder<W, S, Q, N, NoRecordKey> {
    /// Configure the writer to store a user-defined record-key type.
    #[must_use]
    pub fn record_key<K: RecordKey>(self) -> DryIceWriterBuilder<W, S, Q, N, K> {
        DryIceWriterBuilder {
            inner: self.inner,
            target_block_records: self.target_block_records,
            _codec: PhantomData,
            _quality: PhantomData,
            _name: PhantomData,
            _key: PhantomData,
        }
    }

    /// Configure the writer to store the built-in 8-byte key type.
    #[must_use]
    pub fn bytes8_key(self) -> DryIceWriterBuilder<W, S, Q, N, Bytes8Key> {
        self.record_key::<Bytes8Key>()
    }

    /// Configure the writer to store the built-in 16-byte key type.
    #[must_use]
    pub fn bytes16_key(self) -> DryIceWriterBuilder<W, S, Q, N, Bytes16Key> {
        self.record_key::<Bytes16Key>()
    }
}

impl<W: Write, S: SequenceCodec, Q: QualityCodec, N: NameCodec>
    DryIceWriterBuilder<W, S, Q, N, NoRecordKey>
{
    /// Build an unkeyed writer.
    #[must_use]
    pub fn build(self) -> DryIceWriter<W, S, Q, N, NoRecordKey> {
        DryIceWriter::new_unkeyed(self.inner, self.target_block_records)
    }
}

impl<W: Write, S: SequenceCodec, Q: QualityCodec, N: NameCodec, K: RecordKey>
    DryIceWriterBuilder<W, S, Q, N, K>
{
    /// Build a keyed writer.
    #[must_use]
    pub fn build(self) -> DryIceWriter<W, S, Q, N, K> {
        DryIceWriter::new_keyed(self.inner, self.target_block_records)
    }
}

#[cfg(feature = "async")]
impl<W, S: SequenceCodec, Q: QualityCodec, N: NameCodec>
    DryIceWriterBuilder<W, S, Q, N, NoRecordKey>
{
    /// Build an unkeyed async writer.
    #[must_use]
    pub fn build_async(self) -> crate::async_io::AsyncDryIceWriter<W, S, Q, N, NoRecordKey> {
        crate::async_io::AsyncDryIceWriter::new_unkeyed(self.inner, self.target_block_records)
    }
}

#[cfg(feature = "async")]
impl<W, S: SequenceCodec, Q: QualityCodec, N: NameCodec, K: RecordKey>
    DryIceWriterBuilder<W, S, Q, N, K>
{
    /// Build a keyed async writer.
    #[must_use]
    pub fn build_async(self) -> crate::async_io::AsyncDryIceWriter<W, S, Q, N, K> {
        crate::async_io::AsyncDryIceWriter::new_keyed(self.inner, self.target_block_records)
    }
}

/// Writes sequencing records into the `dryice` block-oriented format.
pub struct DryIceWriter<
    W,
    S: SequenceCodec = RawAsciiCodec,
    Q: QualityCodec = RawQualityCodec,
    N: NameCodec = RawNameCodec,
    K = NoRecordKey,
> {
    inner: W,
    target_block_records: usize,
    block_builder: BlockBuilder<S, Q, N>,
    header_written: bool,
    _key: PhantomData<K>,
}

impl DryIceWriter<MissingInner, RawAsciiCodec, RawQualityCodec, RawNameCodec, NoRecordKey> {
    /// Start building a new writer.
    #[must_use]
    pub fn builder()
    -> DryIceWriterBuilder<MissingInner, RawAsciiCodec, RawQualityCodec, RawNameCodec, NoRecordKey>
    {
        DryIceWriterBuilder::new()
    }
}

impl<W, S: SequenceCodec, Q: QualityCodec, N: NameCodec> DryIceWriter<W, S, Q, N, NoRecordKey> {
    fn new_unkeyed(inner: W, target_block_records: usize) -> Self {
        let block_builder = BlockBuilder::new(&BlockBuilderConfig {
            record_key_width: None,
            record_key_tag: None,
            target_records: target_block_records,
        });

        Self {
            inner,
            target_block_records,
            block_builder,
            header_written: false,
            _key: PhantomData,
        }
    }
}

impl<W, S: SequenceCodec, Q: QualityCodec, N: NameCodec, K: RecordKey> DryIceWriter<W, S, Q, N, K> {
    fn new_keyed(inner: W, target_block_records: usize) -> Self {
        let block_builder = BlockBuilder::new(&BlockBuilderConfig {
            record_key_width: Some(K::WIDTH),
            record_key_tag: Some(K::TYPE_TAG),
            target_records: target_block_records,
        });

        Self {
            inner,
            target_block_records,
            block_builder,
            header_written: false,
            _key: PhantomData,
        }
    }
}

impl<W, S: SequenceCodec, Q: QualityCodec, N: NameCodec, K> DryIceWriter<W, S, Q, N, K> {
    fn ensure_header_written(&mut self) -> Result<(), DryIceError>
    where
        W: Write,
    {
        if !self.header_written {
            format::write_file_header(&mut self.inner)?;
            self.header_written = true;
        }
        Ok(())
    }
}

impl<W: Write, S: SequenceCodec, Q: QualityCodec, N: NameCodec>
    DryIceWriter<W, S, Q, N, NoRecordKey>
{
    /// Create an unkeyed writer from a pre-built options struct.
    ///
    /// # Errors
    ///
    /// Returns an error if the options request an unsupported block-size policy.
    pub fn from_options(inner: W, options: &DryIceWriterOptions) -> Result<Self, DryIceError> {
        let target_block_records = match options.layout.block_size {
            BlockSizePolicy::TargetRecords(n) => n,
            BlockSizePolicy::TargetBytes(_) => {
                return Err(DryIceError::InvalidWriterConfiguration(
                    "TargetBytes block size policy is not yet supported",
                ));
            },
        };

        Ok(Self::new_unkeyed(inner, target_block_records))
    }

    /// Assemble the current configuration into an options struct.
    #[must_use]
    pub fn options(&self) -> DryIceWriterOptions {
        DryIceWriterOptions {
            layout: BlockLayoutOptions {
                block_size: BlockSizePolicy::TargetRecords(self.target_block_records),
            },
        }
    }

    /// Write a single sequencing record to an unkeyed writer.
    ///
    /// # Errors
    ///
    /// Returns an error if the record fails validation, if the file header cannot
    /// be written, or if flushing the current block fails.
    pub fn write_record<R: SeqRecordLike>(&mut self, record: &R) -> Result<(), DryIceError> {
        self.ensure_header_written()?;
        self.block_builder.push_record(record)?;

        if self.block_builder.should_flush() {
            self.flush_block()?;
        }

        Ok(())
    }
}

impl<W: Write, S: SequenceCodec, Q: QualityCodec, N: NameCodec, K: RecordKey>
    DryIceWriter<W, S, Q, N, K>
{
    /// Write a single sequencing record together with its accelerator key.
    ///
    /// # Errors
    ///
    /// Returns an error if the record fails validation, if the key cannot be
    /// encoded, if the file header cannot be written, or if flushing the current
    /// block fails.
    pub fn write_record_with_key<R: SeqRecordLike>(
        &mut self,
        record: &R,
        key: &K,
    ) -> Result<(), DryIceError> {
        self.ensure_header_written()?;
        self.block_builder.push_record_with_key(record, key)?;

        if self.block_builder.should_flush() {
            self.flush_block()?;
        }

        Ok(())
    }
}

impl<W: Write, S: SequenceCodec, Q: QualityCodec, N: NameCodec, K> DryIceWriter<W, S, Q, N, K> {
    /// Flush any remaining buffered records and finalize the file.
    ///
    /// # Errors
    ///
    /// Returns an error if writing the file header or flushing the final block
    /// fails.
    pub fn finish(mut self) -> Result<W, DryIceError> {
        self.ensure_header_written()?;

        if !self.block_builder.is_empty() {
            self.flush_block()?;
        }

        Ok(self.inner)
    }

    fn flush_block(&mut self) -> Result<(), DryIceError> {
        self.block_builder.write_block(&mut self.inner)?;
        Ok(())
    }
}

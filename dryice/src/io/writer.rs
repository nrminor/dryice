//! Writer for the `dryice` format.

use std::{io::Write, marker::PhantomData};

use crate::{
    block::sequence::{RawAsciiCodec, SequenceCodec, TwoBitExactCodec},
    block::{BlockBuilder, BlockBuilderConfig},
    codec::{BlockSizePolicy, NameEncoding, QualityEncoding},
    config::{BlockLayoutOptions, DryIceWriterOptions, EncodingOptions},
    error::DryIceError,
    format,
    key::{Bytes8Key, Bytes16Key, NoRecordKey, RecordKey},
    record::SeqRecordLike,
};

/// Private marker type used to track a missing writer target in the builder.
pub struct MissingInner;

/// Builder for [`DryIceWriter`].
pub struct DryIceWriterBuilder<W = MissingInner, S = RawAsciiCodec, K = NoRecordKey> {
    inner: W,
    quality_encoding: QualityEncoding,
    name_encoding: NameEncoding,
    target_block_records: usize,
    _codec: PhantomData<S>,
    _key: PhantomData<K>,
}

impl DryIceWriterBuilder<MissingInner, RawAsciiCodec, NoRecordKey> {
    fn new() -> Self {
        Self {
            inner: MissingInner,
            quality_encoding: QualityEncoding::Raw,
            name_encoding: NameEncoding::Raw,
            target_block_records: 8192,
            _codec: PhantomData,
            _key: PhantomData,
        }
    }
}

impl<W, S, K> DryIceWriterBuilder<W, S, K> {
    /// Set the quality encoding for new blocks.
    #[must_use]
    pub fn quality_encoding(mut self, encoding: QualityEncoding) -> Self {
        self.quality_encoding = encoding;
        self
    }

    /// Set the name encoding for new blocks.
    #[must_use]
    pub fn name_encoding(mut self, encoding: NameEncoding) -> Self {
        self.name_encoding = encoding;
        self
    }

    /// Set the block size policy in records.
    #[must_use]
    pub fn target_block_records(mut self, n: usize) -> Self {
        self.target_block_records = n;
        self
    }
}

impl<S, K> DryIceWriterBuilder<MissingInner, S, K> {
    /// Set the writer's output target.
    #[must_use]
    pub fn inner<W>(self, inner: W) -> DryIceWriterBuilder<W, S, K> {
        DryIceWriterBuilder {
            inner,
            quality_encoding: self.quality_encoding,
            name_encoding: self.name_encoding,
            target_block_records: self.target_block_records,
            _codec: PhantomData,
            _key: PhantomData,
        }
    }
}

impl<W, K> DryIceWriterBuilder<W, RawAsciiCodec, K> {
    /// Configure the writer to use a user-defined sequence codec.
    #[must_use]
    pub fn sequence_codec<S: SequenceCodec>(self) -> DryIceWriterBuilder<W, S, K> {
        DryIceWriterBuilder {
            inner: self.inner,
            quality_encoding: self.quality_encoding,
            name_encoding: self.name_encoding,
            target_block_records: self.target_block_records,
            _codec: PhantomData,
            _key: PhantomData,
        }
    }

    /// Configure the writer to use the built-in 2-bit exact codec.
    #[must_use]
    pub fn two_bit_exact(self) -> DryIceWriterBuilder<W, TwoBitExactCodec, K> {
        self.sequence_codec::<TwoBitExactCodec>()
    }
}

impl<W, S, K> DryIceWriterBuilder<W, S, K>
where
    S: SequenceCodec,
{
    /// Configure the writer to store a user-defined record-key type.
    #[must_use]
    pub fn record_key<K2: RecordKey>(self) -> DryIceWriterBuilder<W, S, K2> {
        DryIceWriterBuilder {
            inner: self.inner,
            quality_encoding: self.quality_encoding,
            name_encoding: self.name_encoding,
            target_block_records: self.target_block_records,
            _codec: PhantomData,
            _key: PhantomData,
        }
    }

    /// Configure the writer to store the built-in 8-byte key type.
    #[must_use]
    pub fn bytes8_key(self) -> DryIceWriterBuilder<W, S, Bytes8Key> {
        self.record_key::<Bytes8Key>()
    }

    /// Configure the writer to store the built-in 16-byte key type.
    #[must_use]
    pub fn bytes16_key(self) -> DryIceWriterBuilder<W, S, Bytes16Key> {
        self.record_key::<Bytes16Key>()
    }
}

impl<W: Write, S: SequenceCodec> DryIceWriterBuilder<W, S, NoRecordKey> {
    /// Build an unkeyed writer.
    #[must_use]
    pub fn build(self) -> DryIceWriter<W, S, NoRecordKey> {
        DryIceWriter::new_unkeyed(
            self.inner,
            self.quality_encoding,
            self.name_encoding,
            self.target_block_records,
        )
    }
}

impl<W: Write, S: SequenceCodec, K: RecordKey> DryIceWriterBuilder<W, S, K> {
    /// Build a keyed writer.
    #[must_use]
    pub fn build(self) -> DryIceWriter<W, S, K> {
        DryIceWriter::new_keyed(
            self.inner,
            self.quality_encoding,
            self.name_encoding,
            self.target_block_records,
        )
    }
}

/// Writes sequencing records into the `dryice` block-oriented format.
pub struct DryIceWriter<W, S = RawAsciiCodec, K = NoRecordKey> {
    inner: W,
    quality_encoding: QualityEncoding,
    name_encoding: NameEncoding,
    target_block_records: usize,
    block_builder: BlockBuilder,
    header_written: bool,
    _codec: PhantomData<S>,
    _key: PhantomData<K>,
}

impl DryIceWriter<MissingInner, RawAsciiCodec, NoRecordKey> {
    /// Start building a new writer.
    #[must_use]
    pub fn builder() -> DryIceWriterBuilder<MissingInner, RawAsciiCodec, NoRecordKey> {
        DryIceWriterBuilder::new()
    }
}

impl<W, S: SequenceCodec> DryIceWriter<W, S, NoRecordKey> {
    fn new_unkeyed(
        inner: W,
        quality_encoding: QualityEncoding,
        name_encoding: NameEncoding,
        target_block_records: usize,
    ) -> Self {
        let block_builder = BlockBuilder::new(&BlockBuilderConfig {
            sequence_encoding: S::ENCODING_TAG,
            quality_encoding,
            name_encoding,
            record_key_width: None,
            record_key_tag: None,
            target_records: target_block_records,
            sequence_encode_fn: S::encode,
        });

        Self {
            inner,
            quality_encoding,
            name_encoding,
            target_block_records,
            block_builder,
            header_written: false,
            _codec: PhantomData,
            _key: PhantomData,
        }
    }
}

impl<W, S: SequenceCodec, K: RecordKey> DryIceWriter<W, S, K> {
    fn new_keyed(
        inner: W,
        quality_encoding: QualityEncoding,
        name_encoding: NameEncoding,
        target_block_records: usize,
    ) -> Self {
        let block_builder = BlockBuilder::new(&BlockBuilderConfig {
            sequence_encoding: S::ENCODING_TAG,
            quality_encoding,
            name_encoding,
            record_key_width: Some(K::WIDTH),
            record_key_tag: Some(K::TYPE_TAG),
            target_records: target_block_records,
            sequence_encode_fn: S::encode,
        });

        Self {
            inner,
            quality_encoding,
            name_encoding,
            target_block_records,
            block_builder,
            header_written: false,
            _codec: PhantomData,
            _key: PhantomData,
        }
    }
}

impl<W, S, K> DryIceWriter<W, S, K> {
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

impl<W: Write, S: SequenceCodec> DryIceWriter<W, S, NoRecordKey> {
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

        Ok(Self::new_unkeyed(
            inner,
            options.encoding.quality,
            options.encoding.names,
            target_block_records,
        ))
    }

    /// Assemble the current configuration into an unkeyed options struct.
    #[must_use]
    pub fn options(&self) -> DryIceWriterOptions {
        DryIceWriterOptions {
            encoding: EncodingOptions {
                sequence: S::ENCODING_TAG,
                quality: self.quality_encoding,
                names: self.name_encoding,
            },
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

impl<W: Write, S: SequenceCodec, K: RecordKey> DryIceWriter<W, S, K> {
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

impl<W: Write, S, K> DryIceWriter<W, S, K> {
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

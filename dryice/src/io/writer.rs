//! Writer for the `dryice` format.

use std::io::Write;

use crate::{
    block::BlockBuilder,
    codec::{BlockSizePolicy, NameEncoding, QualityEncoding, SequenceEncoding, SortKeyKind},
    config::{BlockLayoutOptions, DryIceWriterOptions, EncodingOptions},
    error::DryIceError,
    format,
    record::SeqRecordLike,
};

/// Writes sequencing records into the `dryice` block-oriented format.
///
/// The writer accepts any type implementing [`SeqRecordLike`] and
/// assembles records into blocks internally based on the configured
/// block size policy and encoding options.
///
/// # Construction
///
/// Use the builder to configure the writer. All options have sensible
/// defaults, so the minimal path is:
///
/// ```no_run
/// use dryice::DryIceWriter;
///
/// # fn example() -> Result<(), dryice::DryIceError> {
/// let file = std::fs::File::create("reads.dryice")?;
/// let mut writer = DryIceWriter::builder()
///     .inner(file)
///     .build();
/// # Ok(())
/// # }
/// ```
///
/// A more explicit configuration:
///
/// ```no_run
/// use dryice::{DryIceWriter, SequenceEncoding, QualityEncoding};
///
/// # fn example() -> Result<(), dryice::DryIceError> {
/// let file = std::fs::File::create("reads.dryice")?;
/// let mut writer = DryIceWriter::builder()
///     .inner(file)
///     .sequence_encoding(SequenceEncoding::TwoBitExact)
///     .quality_encoding(QualityEncoding::Binned)
///     .target_block_records(4096)
///     .build();
/// # Ok(())
/// # }
/// ```
pub struct DryIceWriter<W> {
    inner: W,
    sequence_encoding: SequenceEncoding,
    quality_encoding: QualityEncoding,
    name_encoding: NameEncoding,
    sort_key: Option<SortKeyKind>,
    target_block_records: usize,
    block_builder: BlockBuilder,
    header_written: bool,
}

#[bon::bon]
impl<W> DryIceWriter<W> {
    /// Start building a new writer.
    #[builder]
    pub fn new(
        inner: W,
        #[builder(default = SequenceEncoding::RawAscii)] sequence_encoding: SequenceEncoding,
        #[builder(default = QualityEncoding::Raw)] quality_encoding: QualityEncoding,
        #[builder(default = NameEncoding::Raw)] name_encoding: NameEncoding,
        sort_key: Option<SortKeyKind>,
        #[builder(default = 8192)] target_block_records: usize,
    ) -> Self {
        Self {
            inner,
            sequence_encoding,
            quality_encoding,
            name_encoding,
            sort_key,
            target_block_records,
            block_builder: BlockBuilder::new(
                sequence_encoding,
                quality_encoding,
                name_encoding,
                sort_key,
                target_block_records,
            ),
            header_written: false,
        }
    }
}

impl<W: Write> DryIceWriter<W> {
    /// Create a writer from a pre-built options struct.
    ///
    /// Most users should prefer the builder API instead.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid or if the
    /// file header cannot be written.
    pub fn from_options(inner: W, options: &DryIceWriterOptions) -> Result<Self, DryIceError> {
        let target_block_records = match options.layout.block_size {
            BlockSizePolicy::TargetRecords(n) => n,
            BlockSizePolicy::TargetBytes(_) => {
                return Err(DryIceError::InvalidWriterConfiguration(
                    "TargetBytes block size policy is not yet supported",
                ));
            },
        };

        Ok(Self::builder()
            .inner(inner)
            .sequence_encoding(options.encoding.sequence)
            .quality_encoding(options.encoding.quality)
            .name_encoding(options.encoding.names)
            .maybe_sort_key(options.sort_key)
            .target_block_records(target_block_records)
            .build())
    }

    /// Assemble the current configuration into a [`DryIceWriterOptions`].
    #[must_use]
    pub fn options(&self) -> DryIceWriterOptions {
        DryIceWriterOptions {
            encoding: EncodingOptions {
                sequence: self.sequence_encoding,
                quality: self.quality_encoding,
                names: self.name_encoding,
            },
            layout: BlockLayoutOptions {
                block_size: BlockSizePolicy::TargetRecords(self.target_block_records),
            },
            sort_key: self.sort_key,
        }
    }

    fn ensure_header_written(&mut self) -> Result<(), DryIceError> {
        if !self.header_written {
            format::write_file_header(&mut self.inner)?;
            self.header_written = true;
        }
        Ok(())
    }

    /// Write a single sequencing record.
    ///
    /// The record is appended to the current block. When the block
    /// reaches the configured size threshold, it is automatically
    /// flushed to the underlying writer.
    ///
    /// # Errors
    ///
    /// Returns an error if the record fails validation or if an I/O
    /// error occurs during a block flush.
    pub fn write_record<R: SeqRecordLike>(&mut self, record: &R) -> Result<(), DryIceError> {
        self.ensure_header_written()?;
        self.block_builder.push_record(record)?;

        if self.block_builder.should_flush() {
            self.flush_block()?;
        }

        Ok(())
    }

    /// Flush any remaining buffered records and finalize the file.
    ///
    /// This must be called to ensure all data is written. Returns the
    /// underlying writer on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the final block flush or file finalization
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

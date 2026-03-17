//! Writer for the `dryice` format.

use std::io::Write;

use bon::Builder;

use crate::block::BlockBuilder;
use crate::codec::{BlockSizePolicy, NameEncoding, QualityEncoding, SequenceEncoding, SortKeyKind};
use crate::config::{BlockLayoutOptions, DryIceWriterOptions, EncodingOptions};
use crate::error::DryIceError;
use crate::record::SeqRecordLike;

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
///     .build()?;
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
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Builder)]
#[builder(on(String, into), on(Vec<u8>, into))]
pub struct DryIceWriter<W> {
    inner: W,

    #[builder(default = SequenceEncoding::RawAscii)]
    sequence_encoding: SequenceEncoding,

    #[builder(default = QualityEncoding::Raw)]
    quality_encoding: QualityEncoding,

    #[builder(default = NameEncoding::Raw)]
    name_encoding: NameEncoding,

    sort_key: Option<SortKeyKind>,

    #[builder(default = 8192)]
    target_block_records: usize,

    #[builder(skip = BlockBuilder::new(
        SequenceEncoding::RawAscii,
        QualityEncoding::Raw,
        NameEncoding::Raw,
        None,
        8192,
    ))]
    block_builder: BlockBuilder,
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
            BlockSizePolicy::TargetBytes(_) => 8192,
        };

        let block_builder = BlockBuilder::new(
            options.encoding.sequence,
            options.encoding.quality,
            options.encoding.names,
            options.sort_key,
            target_block_records,
        );

        Ok(Self {
            inner,
            sequence_encoding: options.encoding.sequence,
            quality_encoding: options.encoding.quality,
            name_encoding: options.encoding.names,
            sort_key: options.sort_key,
            target_block_records,
            block_builder,
        })
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
        if !self.block_builder.is_empty() {
            self.flush_block()?;
        }

        Ok(self.inner)
    }

    fn flush_block(&mut self) -> Result<(), DryIceError> {
        let encoded = self.block_builder.finish_block()?;
        self.inner.write_all(&encoded)?;
        self.block_builder = BlockBuilder::new(
            self.sequence_encoding,
            self.quality_encoding,
            self.name_encoding,
            self.sort_key,
            self.target_block_records,
        );
        Ok(())
    }
}

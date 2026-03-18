//! Async writer for the `dryice` format.

use std::marker::PhantomData;

use tokio::io::AsyncWriteExt;

use crate::{
    block::{
        BlockBuilder, BlockBuilderConfig,
        name::{NameCodec, RawNameCodec},
        quality::{QualityCodec, RawQualityCodec},
        sequence::{RawAsciiCodec, SequenceCodec},
    },
    error::DryIceError,
    key::{NoRecordKey, RecordKey},
    record::SeqRecordLike,
};

use super::format as async_format;

/// Async writer for the `dryice` block-oriented format.
///
/// This is the async counterpart of [`DryIceWriter`](crate::DryIceWriter).
/// Block building and codec encoding are synchronous; only the I/O
/// operations (header writes, block flushes) are async.
pub struct AsyncDryIceWriter<
    W,
    S: SequenceCodec = RawAsciiCodec,
    Q: QualityCodec = RawQualityCodec,
    N: NameCodec = RawNameCodec,
    K = NoRecordKey,
> {
    inner: W,
    block_builder: BlockBuilder<S, Q, N>,
    flush_buf: Vec<u8>,
    header_written: bool,
    _key: PhantomData<K>,
}

impl<W, S: SequenceCodec, Q: QualityCodec, N: NameCodec>
    AsyncDryIceWriter<W, S, Q, N, NoRecordKey>
{
    pub(crate) fn new_unkeyed(inner: W, target_block_records: usize) -> Self {
        let block_builder = BlockBuilder::new(&BlockBuilderConfig {
            record_key_width: None,
            record_key_tag: None,
            target_records: target_block_records,
        });
        Self {
            inner,
            block_builder,
            flush_buf: Vec::new(),
            header_written: false,
            _key: PhantomData,
        }
    }
}

impl<W, S: SequenceCodec, Q: QualityCodec, N: NameCodec, K: RecordKey>
    AsyncDryIceWriter<W, S, Q, N, K>
{
    pub(crate) fn new_keyed(inner: W, target_block_records: usize) -> Self {
        let block_builder = BlockBuilder::new(&BlockBuilderConfig {
            record_key_width: Some(K::WIDTH),
            record_key_tag: Some(K::TYPE_TAG),
            target_records: target_block_records,
        });
        Self {
            inner,
            block_builder,
            flush_buf: Vec::new(),
            header_written: false,
            _key: PhantomData,
        }
    }
}

impl<W: AsyncWriteExt + Unpin, S: SequenceCodec, Q: QualityCodec, N: NameCodec>
    AsyncDryIceWriter<W, S, Q, N, NoRecordKey>
{
    /// Write a single sequencing record.
    ///
    /// # Errors
    ///
    /// Returns an error if the record fails validation or if an async
    /// I/O error occurs during a block flush.
    pub async fn write_record<R: SeqRecordLike>(&mut self, record: &R) -> Result<(), DryIceError> {
        self.ensure_header_written().await?;
        self.block_builder.push_record(record)?;
        if self.block_builder.should_flush() {
            self.flush_block().await?;
        }
        Ok(())
    }
}

impl<W: AsyncWriteExt + Unpin, S: SequenceCodec, Q: QualityCodec, N: NameCodec, K: RecordKey>
    AsyncDryIceWriter<W, S, Q, N, K>
{
    /// Write a single sequencing record with an accelerator key.
    ///
    /// # Errors
    ///
    /// Returns an error if the record fails validation, if the key
    /// cannot be encoded, or if an async I/O error occurs.
    pub async fn write_record_with_key<R: SeqRecordLike>(
        &mut self,
        record: &R,
        key: &K,
    ) -> Result<(), DryIceError> {
        self.ensure_header_written().await?;
        self.block_builder.push_record_with_key(record, key)?;
        if self.block_builder.should_flush() {
            self.flush_block().await?;
        }
        Ok(())
    }
}

impl<W: AsyncWriteExt + Unpin, S: SequenceCodec, Q: QualityCodec, N: NameCodec, K>
    AsyncDryIceWriter<W, S, Q, N, K>
{
    /// Flush remaining records and finalize the file.
    ///
    /// # Errors
    ///
    /// Returns an error if writing the file header or flushing the
    /// final block fails.
    pub async fn finish(mut self) -> Result<W, DryIceError> {
        self.ensure_header_written().await?;
        if !self.block_builder.is_empty() {
            self.flush_block().await?;
        }
        Ok(self.inner)
    }

    async fn ensure_header_written(&mut self) -> Result<(), DryIceError> {
        if !self.header_written {
            async_format::write_file_header(&mut self.inner).await?;
            self.header_written = true;
        }
        Ok(())
    }

    async fn flush_block(&mut self) -> Result<(), DryIceError> {
        self.flush_buf.clear();
        self.block_builder.write_block(&mut self.flush_buf)?;
        self.inner.write_all(&self.flush_buf).await?;
        Ok(())
    }
}

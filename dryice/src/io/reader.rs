//! Reader for the `dryice` format.

use std::io::Read;

use crate::{
    block::BlockDecoder,
    error::DryIceError,
    format,
    record::{SeqRecord, SeqRecordExt, SeqRecordLike},
};

/// Reads sequencing records from a `dryice` file.
///
/// The reader provides two access patterns:
///
/// **Zero-copy primary path** — the reader itself implements
/// [`SeqRecordLike`], so after calling [`next_record`](Self::next_record),
/// the current record's fields are available as borrowed slices into
/// block-owned buffers with no per-record allocation.
///
/// ```no_run
/// use dryice::{DryIceReader, SeqRecordLike};
///
/// # fn example() -> Result<(), dryice::DryIceError> {
/// let file = std::fs::File::open("reads.dryice")?;
/// let mut reader = DryIceReader::new(file)?;
///
/// while reader.next_record()? {
///     let _seq = reader.sequence();
///     // zero-copy access to block-owned buffers
/// }
/// # Ok(())
/// # }
/// ```
///
/// **Convenience iterator** — for users who prefer `for`-loop syntax
/// and are willing to pay the per-record allocation cost.
///
/// ```no_run
/// use dryice::DryIceReader;
///
/// # fn example() -> Result<(), dryice::DryIceError> {
/// let file = std::fs::File::open("reads.dryice")?;
/// let reader = DryIceReader::new(file)?;
///
/// for record in reader.into_records() {
///     let record = record?;
///     // record is an owned SeqRecord
/// }
/// # Ok(())
/// # }
/// ```
pub struct DryIceReader<R> {
    inner: R,
    current_block: Option<BlockDecoder>,
}

impl<R: Read> DryIceReader<R> {
    /// Open a `dryice` file for reading.
    ///
    /// Parses and validates the file header before returning the reader.
    ///
    /// # Errors
    ///
    /// Returns an error if the file header is missing, corrupt, or
    /// uses an unsupported format version.
    pub fn new(mut inner: R) -> Result<Self, DryIceError> {
        format::read_file_header(&mut inner)?;
        Ok(Self {
            inner,
            current_block: None,
        })
    }

    /// Advance to the next record in the file.
    ///
    /// Returns `true` if a record is now current, `false` at EOF.
    /// After this returns `true`, the reader implements
    /// [`SeqRecordLike`] for the current record — call `name()`,
    /// `sequence()`, and `quality()` to access the fields as borrowed
    /// slices with no allocation.
    ///
    /// # Errors
    ///
    /// Returns an error if a block cannot be read or decoded.
    pub fn next_record(&mut self) -> Result<bool, DryIceError> {
        // Try to advance within the current block.
        if let Some(block) = &mut self.current_block
            && block.advance()
        {
            return Ok(true);
        }

        // Current block is exhausted (or there is none). Load the next.
        loop {
            if let Some(header) = format::read_block_header(&mut self.inner)? {
                let mut decoder = BlockDecoder::from_header_and_reader(header, &mut self.inner)?;
                if decoder.advance() {
                    self.current_block = Some(decoder);
                    return Ok(true);
                }
                // Empty block — try the next one.
            } else {
                self.current_block = None;
                return Ok(false);
            }
        }
    }

    /// Consume this reader into an iterator of owned [`SeqRecord`] values.
    ///
    /// This is a convenience wrapper around [`next_record`](Self::next_record)
    /// that allocates an owned `SeqRecord` for each record. Users who
    /// need maximum throughput should prefer the zero-copy
    /// `next_record()` path instead.
    pub fn into_records(self) -> DryIceRecords<R> {
        DryIceRecords { reader: self }
    }
}

impl<R: Read> SeqRecordLike for DryIceReader<R> {
    /// The current record's name.
    ///
    /// # Panics
    ///
    /// Panics if called before [`next_record`](Self::next_record)
    /// returns `true` or after it returns `false`.
    fn name(&self) -> &[u8] {
        self.current_block
            .as_ref()
            .expect("name() called with no current record")
            .current_name()
    }

    /// The current record's nucleotide sequence.
    ///
    /// # Panics
    ///
    /// Panics if called before [`next_record`](Self::next_record)
    /// returns `true` or after it returns `false`.
    fn sequence(&self) -> &[u8] {
        self.current_block
            .as_ref()
            .expect("sequence() called with no current record")
            .current_sequence()
    }

    /// The current record's per-base quality scores.
    ///
    /// # Panics
    ///
    /// Panics if called before [`next_record`](Self::next_record)
    /// returns `true` or after it returns `false`.
    fn quality(&self) -> &[u8] {
        self.current_block
            .as_ref()
            .expect("quality() called with no current record")
            .current_quality()
    }
}

/// Iterator over records in a `dryice` file, yielding owned
/// [`SeqRecord`] values.
///
/// This is the convenience path for users who prefer `for`-loop
/// syntax. Each record is allocated as an owned `SeqRecord` via
/// [`SeqRecordExt::to_seq_record`]. For zero-copy access, use
/// [`DryIceReader::next_record`] directly instead.
pub struct DryIceRecords<R> {
    reader: DryIceReader<R>,
}

impl<R: Read> Iterator for DryIceRecords<R> {
    type Item = Result<SeqRecord, DryIceError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.reader.next_record() {
            Ok(true) => Some(self.reader.to_seq_record()),
            Ok(false) => None,
            Err(e) => Some(Err(e)),
        }
    }
}

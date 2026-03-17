//! Reader for the `dryice` format.

use std::io::Read;

use crate::block::BlockDecoder;
use crate::error::DryIceError;
use crate::record::SeqRecord;

/// Reads sequencing records from a `dryice` file.
///
/// The reader parses blocks from the underlying byte stream and
/// yields individual records through an iterator interface.
///
/// # Example
///
/// ```no_run
/// use dryice::DryIceReader;
///
/// # fn example() -> Result<(), dryice::DryIceError> {
/// let file = std::fs::File::open("reads.dryice")?;
/// let reader = DryIceReader::new(file)?;
///
/// for record in reader.records() {
///     let record = record?;
///     // use record.sequence(), record.quality(), etc.
/// }
/// # Ok(())
/// # }
/// ```
pub struct DryIceReader<R> {
    #[allow(dead_code)]
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
    pub fn new(inner: R) -> Result<Self, DryIceError> {
        Ok(Self {
            inner,
            current_block: None,
        })
    }

    /// Return an iterator over all records in the file.
    pub fn records(self) -> DryIceRecords<R> {
        DryIceRecords { reader: self }
    }
}

/// Iterator over records in a `dryice` file.
///
/// Yields `Result<SeqRecord, DryIceError>` for each record. Blocks
/// are decoded transparently as the iterator advances.
pub struct DryIceRecords<R> {
    reader: DryIceReader<R>,
}

impl<R: Read> Iterator for DryIceRecords<R> {
    type Item = Result<SeqRecord, DryIceError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(block) = &mut self.reader.current_block
            && !block.is_exhausted()
        {
            return block.next_record().transpose();
        }

        // TODO: read next block from self.reader.inner, parse it into
        // a BlockDecoder, set self.reader.current_block, and yield the
        // first record. Return None at EOF.
        None
    }
}

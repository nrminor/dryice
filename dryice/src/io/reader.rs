//! Reader for the `dryice` format.

use std::{io::Read, marker::PhantomData};

use crate::{
    block::BlockDecoder,
    error::DryIceError,
    format,
    key::{Bytes8Key, Bytes16Key, NoRecordKey, RecordKey},
    record::{SeqRecord, SeqRecordExt, SeqRecordLike},
};

/// Reads sequencing records from a `dryice` file.
///
/// The reader provides two access patterns:
///
/// - a zero-copy primary path via [`next_record`](Self::next_record), where the
///   reader itself implements [`SeqRecordLike`] for the current record
/// - a convenience [`into_records`](Self::into_records) iterator that allocates
///   owned [`SeqRecord`] values for `for`-loop ergonomics
pub struct DryIceReader<R, K = NoRecordKey> {
    inner: R,
    current_block: Option<BlockDecoder>,
    _key: PhantomData<K>,
}

impl<R: Read> DryIceReader<R, NoRecordKey> {
    /// Open an unkeyed `dryice` file for reading.
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
            _key: PhantomData,
        })
    }

    /// Open a reader configured for a user-defined record-key type.
    ///
    /// # Errors
    ///
    /// Returns an error if the file header is missing, corrupt, or uses an
    /// unsupported format version.
    pub fn with_record_key<K: RecordKey>(mut inner: R) -> Result<DryIceReader<R, K>, DryIceError> {
        format::read_file_header(&mut inner)?;
        Ok(DryIceReader {
            inner,
            current_block: None,
            _key: PhantomData,
        })
    }

    /// Open a reader configured for the built-in 8-byte key type.
    ///
    /// # Errors
    ///
    /// Returns an error if the file header is missing, corrupt, or uses an
    /// unsupported format version.
    pub fn with_bytes8_key(inner: R) -> Result<DryIceReader<R, Bytes8Key>, DryIceError> {
        Self::with_record_key::<Bytes8Key>(inner)
    }

    /// Open a reader configured for the built-in 16-byte key type.
    ///
    /// # Errors
    ///
    /// Returns an error if the file header is missing, corrupt, or uses an
    /// unsupported format version.
    pub fn with_bytes16_key(inner: R) -> Result<DryIceReader<R, Bytes16Key>, DryIceError> {
        Self::with_record_key::<Bytes16Key>(inner)
    }
}

impl<R: Read, K: RecordKey> DryIceReader<R, K> {
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

impl<R: Read, K> DryIceReader<R, K> {
    /// Advance to the next record in the file.
    ///
    /// # Errors
    ///
    /// Returns an error if a block header or block payload cannot be read or
    /// decoded.
    pub fn next_record(&mut self) -> Result<bool, DryIceError> {
        if let Some(block) = &mut self.current_block
            && block.advance()?
        {
            return Ok(true);
        }

        loop {
            if let Some(header) = format::read_block_header(&mut self.inner)? {
                let mut decoder = BlockDecoder::from_header_and_reader(header, &mut self.inner)?;
                if decoder.advance()? {
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
    pub fn into_records(self) -> DryIceRecords<R, K> {
        DryIceRecords { reader: self }
    }
}

impl<R: Read, K> SeqRecordLike for DryIceReader<R, K> {
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
pub struct DryIceRecords<R, K = NoRecordKey> {
    reader: DryIceReader<R, K>,
}

impl<R: Read, K> Iterator for DryIceRecords<R, K> {
    type Item = Result<SeqRecord, DryIceError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.reader.next_record() {
            Ok(true) => Some(self.reader.to_seq_record()),
            Ok(false) => None,
            Err(e) => Some(Err(e)),
        }
    }
}

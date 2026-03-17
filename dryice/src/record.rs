//! Sequencing record trait and owned record type.
//!
//! This module defines the two main record-facing abstractions in the
//! crate: [`SeqRecordLike`], the trait that any sequencing record type
//! can implement to be written into a `dryice` file, and [`SeqRecord`],
//! the crate-provided owned row-wise record type returned by the reader.

use crate::error::DryIceError;

/// A read-like sequencing record with name, sequence, and quality fields.
///
/// This is the primary write-side interoperability boundary for `dryice`.
/// Any type that can provide borrowed byte slices for its name, sequence,
/// and quality fields can implement this trait and be written directly
/// into a `dryice` file without conversion into a crate-owned type.
///
/// # Example
///
/// ```
/// use dryice::SeqRecordLike;
///
/// struct MyRecord {
///     name: Vec<u8>,
///     seq: Vec<u8>,
///     qual: Vec<u8>,
/// }
///
/// impl SeqRecordLike for MyRecord {
///     fn name(&self) -> &[u8] { &self.name }
///     fn sequence(&self) -> &[u8] { &self.seq }
///     fn quality(&self) -> &[u8] { &self.qual }
/// }
/// ```
pub trait SeqRecordLike {
    /// The record name or identifier.
    fn name(&self) -> &[u8];

    /// The nucleotide sequence.
    fn sequence(&self) -> &[u8];

    /// The per-base quality scores.
    fn quality(&self) -> &[u8];

    /// The length of the sequence.
    fn len(&self) -> usize {
        self.sequence().len()
    }

    /// Whether the sequence is empty.
    fn is_empty(&self) -> bool {
        self.sequence().is_empty()
    }
}

/// An owned, row-wise sequencing record.
///
/// This is the primary read-side output type for `dryice`. It is returned
/// by the reader's record iterator and can also be constructed directly
/// for testing or interop purposes.
///
/// Fields are private and accessed through methods. Construction goes
/// through invariant-preserving constructors that enforce constraints
/// such as matching sequence and quality lengths.
///
/// `SeqRecord` implements [`SeqRecordLike`], so it can be passed back
/// into a writer without conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeqRecord {
    name: Vec<u8>,
    sequence: Vec<u8>,
    quality: Vec<u8>,
}

impl SeqRecord {
    /// Create a new record from owned byte vectors.
    ///
    /// # Errors
    ///
    /// Returns [`DryIceError::MismatchedSequenceAndQualityLengths`] if
    /// the sequence and quality vectors have different lengths.
    pub fn new(name: Vec<u8>, sequence: Vec<u8>, quality: Vec<u8>) -> Result<Self, DryIceError> {
        if sequence.len() != quality.len() {
            return Err(DryIceError::MismatchedSequenceAndQualityLengths {
                sequence_len: sequence.len(),
                quality_len: quality.len(),
            });
        }

        Ok(Self {
            name,
            sequence,
            quality,
        })
    }

    /// Create a new record by copying from byte slices.
    ///
    /// # Errors
    ///
    /// Returns [`DryIceError::MismatchedSequenceAndQualityLengths`] if
    /// the sequence and quality slices have different lengths.
    pub fn from_slices(name: &[u8], sequence: &[u8], quality: &[u8]) -> Result<Self, DryIceError> {
        Self::new(name.to_vec(), sequence.to_vec(), quality.to_vec())
    }

    /// The record name or identifier.
    #[must_use]
    pub fn name(&self) -> &[u8] {
        &self.name
    }

    /// The nucleotide sequence.
    #[must_use]
    pub fn sequence(&self) -> &[u8] {
        &self.sequence
    }

    /// The per-base quality scores.
    #[must_use]
    pub fn quality(&self) -> &[u8] {
        &self.quality
    }

    /// Consume the record and return the name bytes.
    #[must_use]
    pub fn into_name(self) -> Vec<u8> {
        self.name
    }

    /// Consume the record and return the sequence bytes.
    #[must_use]
    pub fn into_sequence(self) -> Vec<u8> {
        self.sequence
    }

    /// Consume the record and return the quality bytes.
    #[must_use]
    pub fn into_quality(self) -> Vec<u8> {
        self.quality
    }

    /// Consume the record and return all three fields.
    #[must_use]
    pub fn into_parts(self) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        (self.name, self.sequence, self.quality)
    }

    /// The record name as a UTF-8 string, if valid.
    ///
    /// # Errors
    ///
    /// Returns [`std::str::Utf8Error`] if the name bytes are not valid UTF-8.
    pub fn name_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.name)
    }

    /// The nucleotide sequence as a UTF-8 string, if valid.
    ///
    /// # Errors
    ///
    /// Returns [`std::str::Utf8Error`] if the sequence bytes are not valid UTF-8.
    pub fn sequence_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.sequence)
    }

    /// The quality scores as a UTF-8 string, if valid.
    ///
    /// # Errors
    ///
    /// Returns [`std::str::Utf8Error`] if the quality bytes are not valid UTF-8.
    pub fn quality_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.quality)
    }
}

impl std::fmt::Display for SeqRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = std::str::from_utf8(&self.name).unwrap_or("<non-utf8>");
        let seq = std::str::from_utf8(&self.sequence).unwrap_or("<non-utf8>");
        write!(f, "{name}\t{seq}\t({} bp)", self.sequence.len())
    }
}

impl SeqRecordLike for SeqRecord {
    fn name(&self) -> &[u8] {
        self.name()
    }

    fn sequence(&self) -> &[u8] {
        self.sequence()
    }

    fn quality(&self) -> &[u8] {
        self.quality()
    }
}

/// Extension trait providing convenience methods for any [`SeqRecordLike`]
/// implementor.
///
/// This trait is automatically implemented for all types that implement
/// `SeqRecordLike`. It provides higher-level operations such as
/// conversion into the crate's owned [`SeqRecord`] type.
pub trait SeqRecordExt: SeqRecordLike {
    /// Convert this record into an owned [`SeqRecord`] by copying the
    /// field data.
    ///
    /// # Errors
    ///
    /// Returns [`DryIceError::MismatchedSequenceAndQualityLengths`] if
    /// the sequence and quality slices have different lengths.
    fn to_seq_record(&self) -> Result<SeqRecord, DryIceError> {
        SeqRecord::from_slices(self.name(), self.sequence(), self.quality())
    }
}

impl<T: SeqRecordLike + ?Sized> SeqRecordExt for T {}

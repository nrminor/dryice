//! Error types for the `dryice` crate.

use thiserror::Error;

/// Top-level error type for all `dryice` operations.
///
/// This enum is organized into broad categories: transport/IO errors,
/// configuration errors, input-record validity errors, format identity
/// and version errors, structural corruption errors, unsupported
/// feature errors, and integrity/decode failures.
#[derive(Debug, Error)]
pub enum DryIceError {
    /// An underlying I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The writer configuration is invalid.
    #[error("invalid writer configuration: {0}")]
    InvalidWriterConfiguration(&'static str),

    /// The reader configuration is invalid.
    #[error("invalid reader configuration: {0}")]
    InvalidReaderConfiguration(&'static str),

    /// Sequence and quality lengths do not match.
    #[error("sequence and quality lengths differ: sequence={sequence_len}, quality={quality_len}")]
    MismatchedSequenceAndQualityLengths {
        /// Length of the sequence field.
        sequence_len: usize,
        /// Length of the quality field.
        quality_len: usize,
    },

    /// A required record field is missing or empty.
    #[error("record is missing required field: {field}")]
    MissingRequiredField {
        /// Name of the missing field.
        field: &'static str,
    },

    /// Sequence data is not valid for the selected encoding.
    #[error("invalid sequence encoding input: {message}")]
    InvalidSequenceInput {
        /// Description of the problem.
        message: &'static str,
    },

    /// Quality data is not valid for the selected encoding.
    #[error("invalid quality encoding input: {message}")]
    InvalidQualityInput {
        /// Description of the problem.
        message: &'static str,
    },

    /// The file uses a format version this build does not support.
    #[error("unsupported format version: {version}")]
    UnsupportedFormatVersion {
        /// The version number found in the file header.
        version: u32,
    },

    /// The file does not begin with valid `dryice` magic bytes.
    #[error("invalid file magic bytes")]
    InvalidMagic,

    /// A block header could not be parsed.
    #[error("corrupt block header: {message}")]
    CorruptBlockHeader {
        /// Description of the corruption.
        message: &'static str,
    },

    /// Block layout metadata is inconsistent or unreadable.
    #[error("corrupt block layout: {message}")]
    CorruptBlockLayout {
        /// Description of the corruption.
        message: &'static str,
    },

    /// A record index entry is corrupt or out of range.
    #[error("corrupt record index at entry {entry}: {message}")]
    CorruptRecordIndex {
        /// Zero-based index of the problematic entry.
        entry: usize,
        /// Description of the corruption.
        message: &'static str,
    },

    /// A section that should be present in this block is missing.
    #[error("section `{section}` is missing but required by this block")]
    MissingRequiredSection {
        /// Name of the missing section.
        section: &'static str,
    },

    /// A section is present but should not be for this block configuration.
    #[error("section `{section}` is present but not valid for this block")]
    UnexpectedSection {
        /// Name of the unexpected section.
        section: &'static str,
    },

    /// The block uses a sequence encoding this build does not support.
    #[error("unsupported sequence encoding: {encoding:?}")]
    UnsupportedSequenceEncoding {
        /// The unsupported encoding.
        encoding: crate::codec::SequenceEncoding,
    },

    /// The block uses a quality encoding this build does not support.
    #[error("unsupported quality encoding: {encoding:?}")]
    UnsupportedQualityEncoding {
        /// The unsupported encoding.
        encoding: crate::codec::QualityEncoding,
    },

    /// The block uses a name encoding this build does not support.
    #[error("unsupported name encoding: {encoding:?}")]
    UnsupportedNameEncoding {
        /// The unsupported encoding.
        encoding: crate::codec::NameEncoding,
    },

    /// A block checksum did not match the computed value.
    #[error("block checksum mismatch")]
    ChecksumMismatch,

    /// A record could not be decoded from block data.
    #[error("record {record_index} could not be decoded: {message}")]
    RecordDecode {
        /// Zero-based index of the record within the block.
        record_index: usize,
        /// Description of the decode failure.
        message: &'static str,
    },

    /// A value exceeds the maximum representable size for the format.
    #[error("{field} exceeds u32 range")]
    SectionOverflow {
        /// Which field or section overflowed.
        field: &'static str,
    },
}

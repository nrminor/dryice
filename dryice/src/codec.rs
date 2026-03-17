//! Encoding configuration types for sequence, quality, and name data.
//!
//! These enums represent the user-facing encoding choices available
//! when writing a `dryice` file. The actual codec implementations
//! are internal to the crate.

/// Sequence encoding strategy for a block.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SequenceEncoding {
    /// Store sequences as raw ASCII bytes. Fastest encode/decode,
    /// largest on-disk footprint.
    #[default]
    RawAscii,

    /// Pack canonical bases (A, C, G, T) into 2 bits each with an
    /// ambiguity side channel for exact reconstruction of IUPAC symbols.
    TwoBitExact,

    /// Pack all bases into 2 bits, collapsing ambiguous symbols to a
    /// canonical placeholder. This is explicitly lossy.
    TwoBitLossyN,
}

/// Quality score encoding strategy for a block.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QualityEncoding {
    /// Store quality scores as raw ASCII bytes.
    #[default]
    Raw,

    /// Bin quality scores into a smaller set of representative values.
    /// Lossy but cheap.
    Binned,

    /// Omit quality scores entirely for this block.
    Omitted,
}

/// Name encoding strategy for a block.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NameEncoding {
    /// Store names as raw bytes.
    #[default]
    Raw,

    /// Omit names entirely for this block.
    Omitted,
}

/// Block size policy for the writer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockSizePolicy {
    /// Target a specific number of records per block.
    TargetRecords(usize),

    /// Target a specific approximate byte size per block.
    TargetBytes(usize),
}

/// The kind of sort key stored in an accelerator section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortKeyKind {
    /// A 64-bit minimizer-derived sort key.
    U64Minimizer,

    /// A 128-bit minimizer-derived sort key.
    U128Minimizer,
}

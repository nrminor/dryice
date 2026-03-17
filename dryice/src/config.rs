//! Writer and reader configuration types.
//!
//! This module defines the configuration objects that control how a
//! `dryice` file is written. The underlying config is organized into
//! logical groups, but the primary user-facing construction path is
//! the flat builder on [`DryIceWriter`](crate::DryIceWriter).

use crate::codec::{BlockSizePolicy, NameEncoding, QualityEncoding, SequenceEncoding, SortKeyKind};

/// Top-level configuration for a [`DryIceWriter`](crate::DryIceWriter).
///
/// This struct groups encoding choices, block layout policy, and
/// optional accelerator configuration. Users typically do not
/// construct this directly — instead, use the builder on
/// `DryIceWriter`.
#[derive(Debug, Clone, Default)]
pub struct DryIceWriterOptions {
    /// Encoding choices for sequence, quality, and name data.
    pub encoding: EncodingOptions,

    /// Block layout and sizing policy.
    pub layout: BlockLayoutOptions,

    /// Optional sort key to store as an accelerator section.
    pub sort_key: Option<SortKeyKind>,
}

/// Encoding choices for the three record field types.
#[derive(Debug, Clone, Default)]
pub struct EncodingOptions {
    /// How nucleotide sequences are encoded within blocks.
    pub sequence: SequenceEncoding,

    /// How quality scores are encoded within blocks.
    pub quality: QualityEncoding,

    /// How record names are encoded within blocks.
    pub names: NameEncoding,
}

/// Block layout and sizing policy.
#[derive(Debug, Clone)]
pub struct BlockLayoutOptions {
    /// How the writer decides when to flush a block.
    pub block_size: BlockSizePolicy,
}

impl Default for BlockLayoutOptions {
    fn default() -> Self {
        Self {
            block_size: BlockSizePolicy::TargetRecords(8192),
        }
    }
}

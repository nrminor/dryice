//! Writer and reader configuration types.
//!
//! This module defines the configuration objects that control how a
//! `dryice` file is written. The primary user-facing construction path
//! is the builder on [`DryIceWriter`](crate::DryIceWriter). Sequence,
//! quality, and name codecs are selected via type parameters on the
//! builder, not through this config.

/// Top-level configuration for a [`DryIceWriter`](crate::DryIceWriter).
///
/// This struct carries block layout policy. Sequence, quality, and name
/// codecs are selected via type parameters on the writer builder, not
/// through this struct.
#[derive(Debug, Clone, Default)]
pub struct DryIceWriterOptions {
    /// Block layout and sizing policy.
    pub layout: BlockLayoutOptions,
}

/// Block size policy for the writer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockSizePolicy {
    /// Target a specific number of records per block.
    TargetRecords(usize),

    /// Target a specific approximate byte size per block.
    TargetBytes(usize),
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

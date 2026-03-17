//! Writer and reader configuration types.
//!
//! This module defines the configuration objects that control how a
//! `dryice` file is written. The primary user-facing construction path
//! is the builder on [`DryIceWriter`](crate::DryIceWriter). Sequence
//! and quality codecs are selected via type parameters on the builder,
//! not through this config.

use crate::codec::{BlockSizePolicy, NameEncoding};

/// Top-level configuration for a [`DryIceWriter`](crate::DryIceWriter).
///
/// This struct carries name encoding and block layout policy. Sequence
/// and quality codecs are selected via type parameters on the writer
/// builder, not through this struct.
#[derive(Debug, Clone, Default)]
pub struct DryIceWriterOptions {
    /// How record names are encoded within blocks.
    pub name_encoding: NameEncoding,

    /// Block layout and sizing policy.
    pub layout: BlockLayoutOptions,
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

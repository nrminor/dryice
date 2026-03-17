//! Block-level types and machinery.
//!
//! This module defines the internal block schema that sits at the
//! structural heart of the `dryice` format. A `dryice` file is a
//! sequence of self-contained blocks, each holding a batch of
//! sequencing records with separate payload streams for names,
//! sequences, and qualities.
//!
//! All types in this module are crate-internal.

mod builder;
mod decoder;
pub(crate) mod header;
mod index;
pub mod name;
pub mod quality;
pub mod sequence;

pub(crate) use builder::{BlockBuilder, BlockBuilderConfig};
pub(crate) use decoder::BlockDecoder;

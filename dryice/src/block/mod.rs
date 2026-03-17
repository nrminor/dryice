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
mod header;
mod index;

pub(crate) use builder::BlockBuilder;
pub(crate) use decoder::BlockDecoder;

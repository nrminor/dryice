//! Reader and writer types for the `dryice` format.
//!
//! This module contains the primary operational API for reading and
//! writing `dryice` files. The writer accepts any type implementing
//! [`SeqRecordLike`](crate::SeqRecordLike) and assembles records into
//! blocks internally. The reader exposes the current record as borrowed
//! slices via [`SeqRecordLike`](crate::SeqRecordLike) for zero-copy
//! access, with an optional owned-record iterator for convenience.

mod reader;
mod writer;

pub use reader::{DryIceReader, DryIceRecords};
pub use writer::DryIceWriter;

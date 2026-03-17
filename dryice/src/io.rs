//! Reader and writer types for the `dryice` format.
//!
//! This module contains the primary operational API for reading and
//! writing `dryice` files. The writer accepts any type implementing
//! [`SeqRecordLike`](crate::SeqRecordLike) and assembles records into
//! blocks internally. The reader yields owned [`SeqRecord`](crate::SeqRecord)
//! values through an iterator interface.

mod reader;
mod writer;

pub use reader::{DryIceReader, DryIceRecords};
pub use writer::DryIceWriter;

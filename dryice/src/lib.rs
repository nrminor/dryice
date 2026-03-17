//! High-throughput transient container for read-like genomic records.
//!
//! `dryice` is a block-oriented temporary storage format optimized for
//! workflows where sequencing records need to move to disk and back
//! quickly, especially external sorting, partitioning, and other
//! out-of-core genomics pipelines.
//!
//! The crate is parser-agnostic: any type implementing [`SeqRecordLike`]
//! can be written into a `dryice` file, and records are read back as
//! owned [`SeqRecord`] values through an iterator interface.
//!
//! # Writing records
//!
//! ```no_run
//! use dryice::{DryIceWriter, SeqRecordLike};
//!
//! # fn example() -> Result<(), dryice::DryIceError> {
//! let file = std::fs::File::create("reads.dryice")?;
//! let mut writer = DryIceWriter::new(file)?;
//!
//! // writer.write_record(&my_record)?;
//! // writer.finish()?;
//! # Ok(())
//! # }
//! ```
//!
//! # Reading records
//!
//! ```no_run
//! use dryice::DryIceReader;
//!
//! # fn example() -> Result<(), dryice::DryIceError> {
//! let file = std::fs::File::open("reads.dryice")?;
//! let reader = DryIceReader::new(file)?;
//!
//! for record in reader.records() {
//!     let record = record?;
//!     // use record.sequence(), record.quality(), etc.
//! }
//! # Ok(())
//! # }
//! ```

pub mod codec;
mod error;
mod io;
mod record;

pub use codec::{BlockSizePolicy, NameEncoding, QualityEncoding, SequenceEncoding, SortKeyKind};
pub use error::DryIceError;
pub use io::{DryIceReader, DryIceRecords, DryIceWriter};
pub use record::{SeqRecord, SeqRecordExt, SeqRecordLike};

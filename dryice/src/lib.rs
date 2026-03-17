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
//! use dryice::{DryIceWriter, SequenceEncoding, QualityEncoding};
//!
//! # fn example() -> Result<(), dryice::DryIceError> {
//! let file = std::fs::File::create("reads.dryice")?;
//! let mut writer = DryIceWriter::builder()
//!     .inner(file)
//!     .sequence_encoding(SequenceEncoding::TwoBitExact)
//!     .quality_encoding(QualityEncoding::Binned)
//!     .target_block_records(4096)
//!     .build()?;
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
pub mod config;
mod error;
mod io;
mod record;

pub use codec::{BlockSizePolicy, NameEncoding, QualityEncoding, SequenceEncoding, SortKeyKind};
pub use config::{BlockLayoutOptions, DryIceWriterOptions, EncodingOptions};
pub use error::DryIceError;
pub use io::{DryIceReader, DryIceRecords, DryIceWriter};
pub use record::{SeqRecord, SeqRecordExt, SeqRecordLike};

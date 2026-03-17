//! High-throughput transient container for read-like genomic records.
//!
//! `dryice` is a block-oriented temporary storage format optimized for
//! workflows where sequencing records need to move to disk and back
//! quickly, especially external sorting, partitioning, and other
//! out-of-core genomics pipelines.
//!
//! The crate is parser-agnostic: any type implementing [`SeqRecordLike`]
//! can be written into a `dryice` file, and records are read back as
//! borrowed slices with no per-record allocation.
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
//!     .build();
//!
//! // writer.write_record(&my_record)?;
//! // writer.finish()?;
//! # Ok(())
//! # }
//! ```
//!
//! # Reading records (zero-copy)
//!
//! ```no_run
//! use dryice::{DryIceReader, SeqRecordLike};
//!
//! # fn example() -> Result<(), dryice::DryIceError> {
//! let file = std::fs::File::open("reads.dryice")?;
//! let mut reader = DryIceReader::new(file)?;
//!
//! while reader.next_record()? {
//!     let _seq = reader.sequence();
//!     // zero-copy access to block-owned buffers
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Reading records (convenience iterator)
//!
//! ```no_run
//! use dryice::DryIceReader;
//!
//! # fn example() -> Result<(), dryice::DryIceError> {
//! let file = std::fs::File::open("reads.dryice")?;
//! let reader = DryIceReader::new(file)?;
//!
//! for record in reader.into_records() {
//!     let record = record?;
//!     // owned SeqRecord — allocates per record
//! }
//! # Ok(())
//! # }
//! ```

mod block;
pub mod codec;
pub mod config;
mod error;
mod format;
mod io;
pub mod key;
mod record;

pub use block::{
    quality::{BinnedQualityCodec, OmittedQualityCodec, QualityCodec, RawQualityCodec},
    sequence::{RawAsciiCodec, SequenceCodec, TwoBitExactCodec},
};
pub use codec::{BlockSizePolicy, NameEncoding};
pub use config::{BlockLayoutOptions, DryIceWriterOptions};
pub use error::DryIceError;
pub use io::{DryIceReader, DryIceRecords, DryIceWriter};
pub use key::{Bytes8Key, Bytes16Key, NoRecordKey, RecordKey};
pub use record::{SeqRecord, SeqRecordExt, SeqRecordLike};

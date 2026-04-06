//! High-throughput transient container for read-like genomic records.
//!
//! `dryice` is a block-oriented temporary storage format optimized for
//! workflows where sequencing records need to move to disk and back
//! quickly, especially external sorting, partitioning, and other
//! out-of-core genomics pipelines.
//!
//! The crate is parser-agnostic: any type implementing [`SeqRecordLike`]
//! can be written into a `dryice` file, and records are read back as
//! borrowed slices with no per-record allocation. Sequence, quality, and
//! name encodings are selected via trait-based codec type parameters,
//! and users can implement their own codecs.
//!
//! # Writing records (default codecs)
//!
//! ```
//! use dryice::{DryIceWriter, SeqRecord, SeqRecordLike};
//!
//! # fn example() -> Result<(), dryice::DryIceError> {
//! let mut buf = Vec::new();
//! let mut writer = DryIceWriter::builder()
//!     .inner(&mut buf)
//!     .build();
//!
//! let record = SeqRecord::new(
//!     b"read1".to_vec(),
//!     b"ACGTACGT".to_vec(),
//!     b"!!!!!!!!".to_vec(),
//! )?;
//! writer.write_record(&record)?;
//! writer.finish()?;
//! # Ok(())
//! # }
//! ```
//!
//! # Writing with compact codecs
//!
//! ```
//! use dryice::{DryIceWriter, SeqRecord};
//!
//! # fn example() -> Result<(), dryice::DryIceError> {
//! let mut buf = Vec::new();
//! let mut writer = DryIceWriter::builder()
//!     .inner(&mut buf)
//!     .two_bit_exact()
//!     .binned_quality()
//!     .split_names()
//!     .target_block_records(4096)
//!     .build();
//!
//! let record = SeqRecord::new(
//!     b"instrument:run:flowcell 1:N:0:ATCACG".to_vec(),
//!     b"ACGTACGT".to_vec(),
//!     b"!!!!!!!!".to_vec(),
//! )?;
//! writer.write_record(&record)?;
//! writer.finish()?;
//! # Ok(())
//! # }
//! ```
//!
//! # Writing with record keys
//!
//! ```
//! use dryice::{Bytes8Key, DryIceWriter, SeqRecord};
//!
//! # fn example() -> Result<(), dryice::DryIceError> {
//! let mut buf = Vec::new();
//! let mut writer = DryIceWriter::builder()
//!     .inner(&mut buf)
//!     .bytes8_key()
//!     .build();
//!
//! let record = SeqRecord::new(
//!     b"read1".to_vec(),
//!     b"ACGTACGT".to_vec(),
//!     b"!!!!!!!!".to_vec(),
//! )?;
//! let key = Bytes8Key(*b"sortkey!");
//! writer.write_record_with_key(&record, &key)?;
//! writer.finish()?;
//! # Ok(())
//! # }
//! ```
//!
//! # Writing key-only files with empty payload
//!
//! ```
//! use dryice::{Bytes16Key, DryIceWriter};
//!
//! # fn example() -> Result<(), dryice::DryIceError> {
//! let mut buf = Vec::new();
//! let mut writer = DryIceWriter::builder()
//!     .inner(&mut buf)
//!     .bytes16_key()
//!     .empty_payload()
//!     .build();
//!
//! writer.write_key_only(&Bytes16Key(*b"0000000000000001"))?;
//! writer.write_key_only(&Bytes16Key(*b"0000000000000002"))?;
//! writer.finish()?;
//! # Ok(())
//! # }
//! ```
//!
//! # Writing minimizer keys with the builder conveniences
//!
//! ```
//! use dryice::{DryIceWriter, Minimizer64, SeqRecord};
//!
//! # fn example() -> Result<(), dryice::DryIceError> {
//! let mut buf = Vec::new();
//! let mut writer = DryIceWriter::builder()
//!     .inner(&mut buf)
//!     .minimizers_with_sequences()
//!     .build();
//!
//! let record = SeqRecord::new(
//!     b"read1".to_vec(),
//!     b"ACGTGCTCAGAGACTCAGAGGATTACAGTTTACGTGCTCAGAGACTCAGAGGA".to_vec(),
//!     vec![b'!'; 53],
//! )?;
//!
//! if let Some(key) = Minimizer64::<31, 15>::try_from_sequence(record.sequence())? {
//!     writer.write_record_with_key(&record, &key)?;
//! }
//!
//! writer.finish()?;
//! # Ok(())
//! # }
//! ```
//!
//! # Reading records (zero-copy)
//!
//! ```
//! use dryice::{DryIceReader, DryIceWriter, SeqRecord, SeqRecordLike};
//!
//! # fn example() -> Result<(), dryice::DryIceError> {
//! let mut buf = Vec::new();
//! let mut writer = DryIceWriter::builder().inner(&mut buf).build();
//! let record = SeqRecord::new(
//!     b"r1".to_vec(), b"ACGT".to_vec(), b"!!!!".to_vec()
//! )?;
//! writer.write_record(&record)?;
//! writer.finish()?;
//!
//! let mut reader = DryIceReader::new(buf.as_slice())?;
//! while reader.next_record()? {
//!     let _name = reader.name();
//!     let _seq = reader.sequence();
//!     let _qual = reader.quality();
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Reading keys directly
//!
//! ```
//! use dryice::{
//!     Bytes16Key, DryIceReader, DryIceWriter, OmittedNameCodec, OmittedQualityCodec,
//!     OmittedSequenceCodec,
//! };
//!
//! # fn example() -> Result<(), dryice::DryIceError> {
//! let mut buf = Vec::new();
//! let mut writer = DryIceWriter::builder()
//!     .inner(&mut buf)
//!     .bytes16_key()
//!     .empty_payload()
//!     .build();
//! writer.write_key_only(&Bytes16Key(*b"0000000000000001"))?;
//! writer.finish()?;
//!
//! let mut reader = DryIceReader::builder()
//!     .inner(buf.as_slice())
//!     .sequence_codec::<OmittedSequenceCodec>()
//!     .quality_codec::<OmittedQualityCodec>()
//!     .name_codec::<OmittedNameCodec>()
//!     .record_key::<Bytes16Key>()
//!     .build()?;
//!
//! while let Some(key) = reader.next_key()? {
//!     let _ = key;
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Reading records (convenience iterator)
//!
//! ```
//! use dryice::{DryIceReader, DryIceWriter, SeqRecord};
//!
//! # fn example() -> Result<(), dryice::DryIceError> {
//! let mut buf = Vec::new();
//! let mut writer = DryIceWriter::builder().inner(&mut buf).build();
//! let record = SeqRecord::new(
//!     b"r1".to_vec(), b"ACGT".to_vec(), b"!!!!".to_vec()
//! )?;
//! writer.write_record(&record)?;
//! writer.finish()?;
//!
//! let reader = DryIceReader::new(buf.as_slice())?;
//! for record in reader.into_records() {
//!     let record = record?;
//!     println!("{}", record);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Zero-copy reader-to-writer piping
//!
//! ```
//! use dryice::{DryIceReader, DryIceWriter, SeqRecord, SeqRecordLike};
//!
//! # fn example() -> Result<(), dryice::DryIceError> {
//! let mut buf1 = Vec::new();
//! let mut writer1 = DryIceWriter::builder().inner(&mut buf1).build();
//! let record = SeqRecord::new(
//!     b"r1".to_vec(), b"ACGT".to_vec(), b"!!!!".to_vec()
//! )?;
//! writer1.write_record(&record)?;
//! writer1.finish()?;
//!
//! let mut buf2 = Vec::new();
//! let mut reader = DryIceReader::new(buf1.as_slice())?;
//! let mut writer2 = DryIceWriter::builder().inner(&mut buf2).build();
//! while reader.next_record()? {
//!     writer2.write_record(&reader)?;
//! }
//! writer2.finish()?;
//! # Ok(())
//! # }
//! ```
//!
//! # Reading with non-default codecs
//!
//! ```
//! use dryice::{
//!     BinnedQualityCodec, DryIceReader, DryIceWriter, SeqRecord,
//!     SeqRecordLike, SplitNameCodec, TwoBitExactCodec,
//! };
//!
//! # fn example() -> Result<(), dryice::DryIceError> {
//! let mut buf = Vec::new();
//! let mut writer = DryIceWriter::builder()
//!     .inner(&mut buf)
//!     .two_bit_exact()
//!     .binned_quality()
//!     .split_names()
//!     .build();
//! let record = SeqRecord::new(
//!     b"instrument:run 1:N:0".to_vec(),
//!     b"ACGT".to_vec(),
//!     b"!!!!".to_vec(),
//! )?;
//! writer.write_record(&record)?;
//! writer.finish()?;
//!
//! let mut reader = DryIceReader::with_codecs::<
//!     TwoBitExactCodec,
//!     BinnedQualityCodec,
//!     SplitNameCodec,
//! >(buf.as_slice())?;
//! while reader.next_record()? {
//!     let _seq = reader.sequence();
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Custom codec implementation
//!
//! ```
//! use dryice::{DryIceError, SequenceCodec};
//!
//! struct UppercaseCodec;
//!
//! impl SequenceCodec for UppercaseCodec {
//!     const TYPE_TAG: [u8; 16] = *b"demo:seq:upper!!";
//!     const LOSSY: bool = true;
//!
//!     fn encode_into(sequence: &[u8], output: &mut Vec<u8>) -> Result<(), DryIceError> {
//!         output.extend(sequence.iter().map(u8::to_ascii_uppercase));
//!         Ok(())
//!     }
//!
//!     fn decode_into(
//!         encoded: &[u8],
//!         _original_len: usize,
//!         output: &mut Vec<u8>,
//!     ) -> Result<(), DryIceError> {
//!         output.extend_from_slice(encoded);
//!         Ok(())
//!     }
//! }
//! ```

#[cfg(feature = "async")]
pub mod async_io;
mod block;
pub mod config;
mod error;
pub mod fields;
mod format;
mod io;
pub mod key;
#[cfg(feature = "mmap")]
pub mod mmap_io;
mod record;

#[cfg(feature = "async")]
pub use async_io::{AsyncDryIceReader, AsyncDryIceWriter};
pub use block::{
    name::{NameCodec, OmittedNameCodec, RawNameCodec, SplitNameCodec},
    quality::{BinnedQualityCodec, OmittedQualityCodec, QualityCodec, RawQualityCodec},
    sequence::{
        OmittedSequenceCodec, RawAsciiCodec, SequenceCodec, TwoBitExactCodec, TwoBitLossyNCodec,
    },
};
pub use config::{BlockLayoutOptions, BlockSizePolicy, DryIceWriterOptions};
pub use error::DryIceError;
pub use io::{DryIceReader, DryIceRecords, DryIceWriter, SelectedDryIceReader, SelectedRecord};
pub use key::{Bytes8Key, Bytes16Key, KmerKey, Minimizer64, NoRecordKey, PrefixKmer64, RecordKey};
#[cfg(feature = "mmap")]
pub use mmap_io::MmapDryIceReader;
pub use record::{EMPTY_RECORD, EmptyRecord, SeqRecord, SeqRecordExt, SeqRecordLike};

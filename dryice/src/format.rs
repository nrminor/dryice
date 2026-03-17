//! On-disk binary format definitions for `dryice` files.
//!
//! This module defines the constants, serialization, and
//! deserialization logic for the file header and block header.
//! All integer fields in the format are little-endian.

// These functions and constants are not yet called from the writer/reader
// but are exercised by tests and will be wired in during the next phase.
#![allow(dead_code)]

use std::io::{Read, Write};

use crate::block::header::{BlockHeader, ByteRange};
use crate::codec::{NameEncoding, QualityEncoding, SequenceEncoding, SortKeyKind};
use crate::error::DryIceError;

/// Magic bytes at the start of every `dryice` file.
const MAGIC: [u8; 4] = *b"DRYI";

/// Current major version of the format.
const VERSION_MAJOR: u16 = 1;

/// Current minor version of the format.
const VERSION_MINOR: u16 = 0;

/// Total size of the file header in bytes.
const FILE_HEADER_SIZE: usize = 8;

/// Total size of a block header in bytes.
///
/// Layout:
/// ```text
/// [4 bytes]  record_count        u32 le
/// [1 byte]   sequence_encoding   u8
/// [1 byte]   quality_encoding    u8
/// [1 byte]   name_encoding       u8
/// [1 byte]   sort_key_kind       u8
/// [16 bytes] index range         offset u64 le + len u64 le
/// [16 bytes] names range         offset u64 le + len u64 le
/// [16 bytes] sequences range     offset u64 le + len u64 le
/// [16 bytes] qualities range     offset u64 le + len u64 le
/// [16 bytes] sort_keys range     offset u64 le + len u64 le
/// ```
const BLOCK_HEADER_SIZE: usize = 88;

// === Encoding tag constants ===

const SEQ_TAG_RAW_ASCII: u8 = 0;
const SEQ_TAG_TWO_BIT_EXACT: u8 = 1;
const SEQ_TAG_TWO_BIT_LOSSY_N: u8 = 2;

const QUAL_TAG_RAW: u8 = 0;
const QUAL_TAG_BINNED: u8 = 1;
const QUAL_TAG_OMITTED: u8 = 2;

const NAME_TAG_RAW: u8 = 0;
const NAME_TAG_OMITTED: u8 = 1;

const SORT_KEY_TAG_NONE: u8 = 0;
const SORT_KEY_TAG_U64_MINIMIZER: u8 = 1;
const SORT_KEY_TAG_U128_MINIMIZER: u8 = 2;

// === File header ===

/// Write the file header to the given writer.
///
/// # Errors
///
/// Returns an error if the write fails.
pub(crate) fn write_file_header<W: Write>(writer: &mut W) -> Result<(), DryIceError> {
    let mut buf = [0u8; FILE_HEADER_SIZE];
    buf[0..4].copy_from_slice(&MAGIC);
    buf[4..6].copy_from_slice(&VERSION_MAJOR.to_le_bytes());
    buf[6..8].copy_from_slice(&VERSION_MINOR.to_le_bytes());
    writer.write_all(&buf)?;
    Ok(())
}

/// Read and validate the file header from the given reader.
///
/// # Errors
///
/// Returns an error if the magic bytes are invalid or the format
/// version is not supported.
pub(crate) fn read_file_header<R: Read>(reader: &mut R) -> Result<(u16, u16), DryIceError> {
    let mut buf = [0u8; FILE_HEADER_SIZE];
    reader.read_exact(&mut buf)?;

    if buf[0..4] != MAGIC {
        return Err(DryIceError::InvalidMagic);
    }

    let major = u16::from_le_bytes([buf[4], buf[5]]);
    let minor = u16::from_le_bytes([buf[6], buf[7]]);

    if major != VERSION_MAJOR {
        return Err(DryIceError::UnsupportedFormatVersion {
            version: u32::from(major),
        });
    }

    Ok((major, minor))
}

// === Block header ===

fn sequence_encoding_to_tag(enc: SequenceEncoding) -> u8 {
    match enc {
        SequenceEncoding::RawAscii => SEQ_TAG_RAW_ASCII,
        SequenceEncoding::TwoBitExact => SEQ_TAG_TWO_BIT_EXACT,
        SequenceEncoding::TwoBitLossyN => SEQ_TAG_TWO_BIT_LOSSY_N,
    }
}

fn tag_to_sequence_encoding(tag: u8) -> Result<SequenceEncoding, DryIceError> {
    match tag {
        SEQ_TAG_RAW_ASCII => Ok(SequenceEncoding::RawAscii),
        SEQ_TAG_TWO_BIT_EXACT => Ok(SequenceEncoding::TwoBitExact),
        SEQ_TAG_TWO_BIT_LOSSY_N => Ok(SequenceEncoding::TwoBitLossyN),
        _ => Err(DryIceError::CorruptBlockHeader {
            message: format!("unknown sequence encoding tag: {tag}"),
        }),
    }
}

fn quality_encoding_to_tag(enc: QualityEncoding) -> u8 {
    match enc {
        QualityEncoding::Raw => QUAL_TAG_RAW,
        QualityEncoding::Binned => QUAL_TAG_BINNED,
        QualityEncoding::Omitted => QUAL_TAG_OMITTED,
    }
}

fn tag_to_quality_encoding(tag: u8) -> Result<QualityEncoding, DryIceError> {
    match tag {
        QUAL_TAG_RAW => Ok(QualityEncoding::Raw),
        QUAL_TAG_BINNED => Ok(QualityEncoding::Binned),
        QUAL_TAG_OMITTED => Ok(QualityEncoding::Omitted),
        _ => Err(DryIceError::CorruptBlockHeader {
            message: format!("unknown quality encoding tag: {tag}"),
        }),
    }
}

fn name_encoding_to_tag(enc: NameEncoding) -> u8 {
    match enc {
        NameEncoding::Raw => NAME_TAG_RAW,
        NameEncoding::Omitted => NAME_TAG_OMITTED,
    }
}

fn tag_to_name_encoding(tag: u8) -> Result<NameEncoding, DryIceError> {
    match tag {
        NAME_TAG_RAW => Ok(NameEncoding::Raw),
        NAME_TAG_OMITTED => Ok(NameEncoding::Omitted),
        _ => Err(DryIceError::CorruptBlockHeader {
            message: format!("unknown name encoding tag: {tag}"),
        }),
    }
}

fn sort_key_kind_to_tag(kind: Option<SortKeyKind>) -> u8 {
    match kind {
        None => SORT_KEY_TAG_NONE,
        Some(SortKeyKind::U64Minimizer) => SORT_KEY_TAG_U64_MINIMIZER,
        Some(SortKeyKind::U128Minimizer) => SORT_KEY_TAG_U128_MINIMIZER,
    }
}

fn tag_to_sort_key_kind(tag: u8) -> Result<Option<SortKeyKind>, DryIceError> {
    match tag {
        SORT_KEY_TAG_NONE => Ok(None),
        SORT_KEY_TAG_U64_MINIMIZER => Ok(Some(SortKeyKind::U64Minimizer)),
        SORT_KEY_TAG_U128_MINIMIZER => Ok(Some(SortKeyKind::U128Minimizer)),
        _ => Err(DryIceError::CorruptBlockHeader {
            message: format!("unknown sort key tag: {tag}"),
        }),
    }
}

fn write_byte_range(buf: &mut [u8], range: ByteRange) {
    buf[0..8].copy_from_slice(&range.offset.to_le_bytes());
    buf[8..16].copy_from_slice(&range.len.to_le_bytes());
}

fn write_optional_byte_range(buf: &mut [u8], range: Option<ByteRange>) {
    let r = range.unwrap_or(ByteRange { offset: 0, len: 0 });
    write_byte_range(buf, r);
}

fn read_byte_range(buf: &[u8]) -> ByteRange {
    let offset = u64::from_le_bytes([
        buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
    ]);
    let len = u64::from_le_bytes([
        buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15],
    ]);
    ByteRange { offset, len }
}

/// Write a block header to the given writer.
///
/// # Errors
///
/// Returns an error if the write fails.
pub(crate) fn write_block_header<W: Write>(
    writer: &mut W,
    header: &BlockHeader,
) -> Result<(), DryIceError> {
    let mut buf = [0u8; BLOCK_HEADER_SIZE];

    buf[0..4].copy_from_slice(&header.record_count.to_le_bytes());
    buf[4] = sequence_encoding_to_tag(header.sequence_encoding);
    buf[5] = quality_encoding_to_tag(header.quality_encoding);
    buf[6] = name_encoding_to_tag(header.name_encoding);
    buf[7] = sort_key_kind_to_tag(header.sort_key_kind);

    write_byte_range(&mut buf[8..24], header.index);
    write_optional_byte_range(&mut buf[24..40], header.names);
    write_byte_range(&mut buf[40..56], header.sequences);
    write_optional_byte_range(&mut buf[56..72], header.qualities);
    write_optional_byte_range(&mut buf[72..88], header.sort_keys);

    writer.write_all(&buf)?;
    Ok(())
}

/// Read a block header from the given reader.
///
/// Returns `None` at EOF (no more blocks). Returns an error if the
/// header is partially present or corrupt.
///
/// # Errors
///
/// Returns an error if the block header cannot be parsed.
pub(crate) fn read_block_header<R: Read>(
    reader: &mut R,
) -> Result<Option<BlockHeader>, DryIceError> {
    let mut buf = [0u8; BLOCK_HEADER_SIZE];

    match reader.read_exact(&mut buf) {
        Ok(()) => {},
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(DryIceError::Io(e)),
    }

    let record_count = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    let sequence_encoding = tag_to_sequence_encoding(buf[4])?;
    let quality_encoding = tag_to_quality_encoding(buf[5])?;
    let name_encoding = tag_to_name_encoding(buf[6])?;
    let sort_key_kind = tag_to_sort_key_kind(buf[7])?;

    let index = read_byte_range(&buf[8..24]);
    let names_range = read_byte_range(&buf[24..40]);
    let sequences = read_byte_range(&buf[40..56]);
    let qualities_range = read_byte_range(&buf[56..72]);
    let sort_keys_range = read_byte_range(&buf[72..88]);

    let names = if name_encoding == NameEncoding::Omitted {
        None
    } else {
        Some(names_range)
    };

    let qualities = if quality_encoding == QualityEncoding::Omitted {
        None
    } else {
        Some(qualities_range)
    };

    let sort_keys = if sort_key_kind.is_none() {
        None
    } else {
        Some(sort_keys_range)
    };

    Ok(Some(BlockHeader {
        record_count,
        sequence_encoding,
        quality_encoding,
        name_encoding,
        sort_key_kind,
        index,
        names,
        sequences,
        qualities,
        sort_keys,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_header_round_trip() {
        let mut buf = Vec::new();
        write_file_header(&mut buf).expect("write should succeed");
        assert_eq!(buf.len(), FILE_HEADER_SIZE);

        let (major, minor) = read_file_header(&mut buf.as_slice()).expect("read should succeed");
        assert_eq!(major, VERSION_MAJOR);
        assert_eq!(minor, VERSION_MINOR);
    }

    #[test]
    fn file_header_rejects_bad_magic() {
        let buf = b"NOPE\x01\x00\x00\x00";
        let result = read_file_header(&mut buf.as_slice());
        assert!(matches!(result, Err(DryIceError::InvalidMagic)));
    }

    #[test]
    fn block_header_round_trip() {
        let header = BlockHeader {
            record_count: 42,
            sequence_encoding: SequenceEncoding::TwoBitExact,
            quality_encoding: QualityEncoding::Binned,
            name_encoding: NameEncoding::Raw,
            sort_key_kind: Some(SortKeyKind::U128Minimizer),
            index: ByteRange {
                offset: 0,
                len: 100,
            },
            names: Some(ByteRange {
                offset: 100,
                len: 200,
            }),
            sequences: ByteRange {
                offset: 300,
                len: 400,
            },
            qualities: Some(ByteRange {
                offset: 700,
                len: 400,
            }),
            sort_keys: Some(ByteRange {
                offset: 1100,
                len: 84,
            }),
        };

        let mut buf = Vec::new();
        write_block_header(&mut buf, &header).expect("write should succeed");
        assert_eq!(buf.len(), BLOCK_HEADER_SIZE);

        let parsed = read_block_header(&mut buf.as_slice())
            .expect("read should succeed")
            .expect("should not be EOF");

        assert_eq!(parsed.record_count, 42);
        assert_eq!(parsed.sequence_encoding, SequenceEncoding::TwoBitExact);
        assert_eq!(parsed.quality_encoding, QualityEncoding::Binned);
        assert_eq!(parsed.name_encoding, NameEncoding::Raw);
        assert_eq!(parsed.sort_key_kind, Some(SortKeyKind::U128Minimizer));
        assert_eq!(parsed.index, header.index);
        assert_eq!(parsed.names, header.names);
        assert_eq!(parsed.sequences, header.sequences);
        assert_eq!(parsed.qualities, header.qualities);
        assert_eq!(parsed.sort_keys, header.sort_keys);
    }

    #[test]
    fn block_header_round_trip_with_omitted_sections() {
        let header = BlockHeader {
            record_count: 10,
            sequence_encoding: SequenceEncoding::RawAscii,
            quality_encoding: QualityEncoding::Omitted,
            name_encoding: NameEncoding::Omitted,
            sort_key_kind: None,
            index: ByteRange { offset: 0, len: 50 },
            names: None,
            sequences: ByteRange {
                offset: 50,
                len: 100,
            },
            qualities: None,
            sort_keys: None,
        };

        let mut buf = Vec::new();
        write_block_header(&mut buf, &header).expect("write should succeed");

        let parsed = read_block_header(&mut buf.as_slice())
            .expect("read should succeed")
            .expect("should not be EOF");

        assert_eq!(parsed.record_count, 10);
        assert_eq!(parsed.name_encoding, NameEncoding::Omitted);
        assert_eq!(parsed.quality_encoding, QualityEncoding::Omitted);
        assert_eq!(parsed.sort_key_kind, None);
        assert!(parsed.names.is_none());
        assert!(parsed.qualities.is_none());
        assert!(parsed.sort_keys.is_none());
    }

    #[test]
    fn block_header_eof_returns_none() {
        let buf: &[u8] = &[];
        let result = read_block_header(&mut &*buf).expect("should not error on clean EOF");
        assert!(result.is_none());
    }
}

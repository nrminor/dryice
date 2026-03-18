//! On-disk binary format definitions for `dryice` files.
//!
//! This module defines the constants, serialization, and deserialization
//! logic for the file header and block header. All integer fields in the
//! format are little-endian.

use std::io::{Read, Write};

use crate::{
    block::header::{BlockHeader, ByteRange},
    error::DryIceError,
};

/// Magic bytes at the start of every `dryice` file.
pub(crate) const MAGIC: [u8; 4] = *b"DRYI";

/// Current major version of the format.
pub(crate) const VERSION_MAJOR: u16 = 1;

/// Current minor version of the format.
pub(crate) const VERSION_MINOR: u16 = 0;

/// Total size of the file header in bytes.
pub(crate) const FILE_HEADER_SIZE: usize = 8;

/// Total size of a block header in bytes.
///
/// Layout:
/// ```text
/// [4 bytes]  record_count          u32 le
/// [16 bytes] sequence_codec_tag    [u8; 16]
/// [16 bytes] quality_codec_tag     [u8; 16]
/// [16 bytes] name_codec_tag        [u8; 16]
/// [1 byte]   has_record_key        u8
/// [1 byte]   reserved              u8
/// [2 bytes]  record_key_width      u16 le
/// [16 bytes] record_key_tag        [u8; 16]
/// [16 bytes] index range           offset u64 le + len u64 le
/// [16 bytes] names range           offset u64 le + len u64 le
/// [16 bytes] sequences range       offset u64 le + len u64 le
/// [16 bytes] qualities range       offset u64 le + len u64 le
/// [16 bytes] record_keys range     offset u64 le + len u64 le
/// ```
pub(crate) const BLOCK_HEADER_SIZE: usize = 152;

/// Write the file header to the given writer.
pub(crate) fn write_file_header<W: Write>(writer: &mut W) -> Result<(), DryIceError> {
    let mut buf = [0u8; FILE_HEADER_SIZE];
    buf[0..4].copy_from_slice(&MAGIC);
    buf[4..6].copy_from_slice(&VERSION_MAJOR.to_le_bytes());
    buf[6..8].copy_from_slice(&VERSION_MINOR.to_le_bytes());
    writer.write_all(&buf)?;
    Ok(())
}

/// Read and validate the file header from the given reader.
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

fn read_tag16(buf: &[u8]) -> [u8; 16] {
    buf[0..16]
        .try_into()
        .expect("tag slice should have length 16")
}

/// Write a block header to the given writer.
pub(crate) fn write_block_header<W: Write>(
    writer: &mut W,
    header: &BlockHeader,
) -> Result<(), DryIceError> {
    let mut buf = [0u8; BLOCK_HEADER_SIZE];

    buf[0..4].copy_from_slice(&header.record_count.to_le_bytes());
    buf[4..20].copy_from_slice(&header.sequence_codec_tag);
    buf[20..36].copy_from_slice(&header.quality_codec_tag);
    buf[36..52].copy_from_slice(&header.name_codec_tag);
    buf[52] = u8::from(header.record_keys.is_some());
    buf[53] = 0; // reserved
    buf[54..56].copy_from_slice(&header.record_key_width.to_le_bytes());
    buf[56..72].copy_from_slice(&header.record_key_tag);

    write_byte_range(&mut buf[72..88], header.index);
    write_optional_byte_range(&mut buf[88..104], header.names);
    write_byte_range(&mut buf[104..120], header.sequences);
    write_optional_byte_range(&mut buf[120..136], header.qualities);
    write_optional_byte_range(&mut buf[136..152], header.record_keys);

    writer.write_all(&buf)?;
    Ok(())
}

/// Read a block header from the given reader.
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
    let sequence_codec_tag = read_tag16(&buf[4..20]);
    let quality_codec_tag = read_tag16(&buf[20..36]);
    let name_codec_tag = read_tag16(&buf[36..52]);
    let has_record_key = buf[52] != 0;
    // buf[53] is reserved
    let record_key_width = u16::from_le_bytes([buf[54], buf[55]]);
    let record_key_tag = read_tag16(&buf[56..72]);

    let index = read_byte_range(&buf[72..88]);
    let names_range = read_byte_range(&buf[88..104]);
    let sequences = read_byte_range(&buf[104..120]);
    let qualities_range = read_byte_range(&buf[120..136]);
    let record_keys_range = read_byte_range(&buf[136..152]);

    let names = if name_codec_tag == *b"dryi:name:omittd" {
        None
    } else {
        Some(names_range)
    };

    let qualities = if quality_codec_tag == *b"dryi:qual:omittd" {
        None
    } else {
        Some(qualities_range)
    };

    let record_keys = if has_record_key {
        Some(record_keys_range)
    } else {
        None
    };

    Ok(Some(BlockHeader {
        record_count,
        sequence_codec_tag,
        quality_codec_tag,
        name_codec_tag,
        record_key_width,
        record_key_tag,
        index,
        names,
        sequences,
        qualities,
        record_keys,
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
            sequence_codec_tag: *b"dryi:seq:2b-exct",
            quality_codec_tag: *b"dryi:qual:binned",
            name_codec_tag: *b"dryi:name:raw!!!",
            record_key_width: 16,
            record_key_tag: *b"dryi:bytes16:key",
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
            record_keys: Some(ByteRange {
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

        assert_eq!(parsed.record_count, header.record_count);
        assert_eq!(parsed.sequence_codec_tag, header.sequence_codec_tag);
        assert_eq!(parsed.quality_codec_tag, header.quality_codec_tag);
        assert_eq!(parsed.name_codec_tag, header.name_codec_tag);
        assert_eq!(parsed.record_key_width, header.record_key_width);
        assert_eq!(parsed.record_key_tag, header.record_key_tag);
        assert_eq!(parsed.index, header.index);
        assert_eq!(parsed.names, header.names);
        assert_eq!(parsed.sequences, header.sequences);
        assert_eq!(parsed.qualities, header.qualities);
        assert_eq!(parsed.record_keys, header.record_keys);
    }

    #[test]
    fn block_header_round_trip_with_omitted_sections() {
        let header = BlockHeader {
            record_count: 10,
            sequence_codec_tag: *b"dryi:seq:raw-asc",
            quality_codec_tag: *b"dryi:qual:omittd",
            name_codec_tag: *b"dryi:name:omittd",
            record_key_width: 0,
            record_key_tag: [0; 16],
            index: ByteRange { offset: 0, len: 50 },
            names: None,
            sequences: ByteRange {
                offset: 50,
                len: 100,
            },
            qualities: None,
            record_keys: None,
        };

        let mut buf = Vec::new();
        write_block_header(&mut buf, &header).expect("write should succeed");

        let parsed = read_block_header(&mut buf.as_slice())
            .expect("read should succeed")
            .expect("should not be EOF");

        assert_eq!(parsed.record_count, 10);
        assert_eq!(parsed.name_codec_tag, *b"dryi:name:omittd");
        assert_eq!(parsed.quality_codec_tag, *b"dryi:qual:omittd");
        assert_eq!(parsed.record_key_width, 0);
        assert!(parsed.names.is_none());
        assert!(parsed.qualities.is_none());
        assert!(parsed.record_keys.is_none());
    }

    #[test]
    fn block_header_eof_returns_none() {
        let buf: &[u8] = &[];
        let result = read_block_header(&mut &*buf).expect("should not error on clean EOF");
        assert!(result.is_none());
    }
}

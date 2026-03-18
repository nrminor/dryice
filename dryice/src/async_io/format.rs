//! Async versions of file header and block header I/O.

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    block::header::BlockHeader,
    error::DryIceError,
    format::{BLOCK_HEADER_SIZE, FILE_HEADER_SIZE, MAGIC, VERSION_MAJOR},
};

pub(crate) async fn write_file_header<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
) -> Result<(), DryIceError> {
    let mut buf = [0u8; FILE_HEADER_SIZE];
    buf[0..4].copy_from_slice(&MAGIC);
    buf[4..6].copy_from_slice(&crate::format::VERSION_MAJOR.to_le_bytes());
    buf[6..8].copy_from_slice(&crate::format::VERSION_MINOR.to_le_bytes());
    writer.write_all(&buf).await?;
    Ok(())
}

pub(crate) async fn read_file_header<R: AsyncReadExt + Unpin>(
    reader: &mut R,
) -> Result<(u16, u16), DryIceError> {
    let mut buf = [0u8; FILE_HEADER_SIZE];
    reader.read_exact(&mut buf).await?;

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

pub(crate) async fn read_block_header<R: AsyncReadExt + Unpin>(
    reader: &mut R,
) -> Result<Option<BlockHeader>, DryIceError> {
    let mut buf = [0u8; BLOCK_HEADER_SIZE];

    match reader.read_exact(&mut buf).await {
        Ok(_) => {},
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(DryIceError::Io(e)),
    }

    crate::format::read_block_header(&mut buf.as_slice())
}

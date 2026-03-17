//! Record-key types and traits.

use crate::error::DryIceError;

/// A fixed-width accelerator key associated with each record in a block.
///
/// A `RecordKey` defines the full contract needed for `dryice` to store,
/// parse, and expose accelerator-key sections:
///
/// - a fixed encoded width shared by all keys of this type
/// - a stable type tag written into the block header
/// - encoding into bytes for writing
/// - decoding from bytes for reading
///
/// Keys are intended for comparison-friendly accelerator sections such as
/// sort keys, hashes, or other workflow-specific derived record keys.
pub trait RecordKey: Ord + Sized {
    /// Width in bytes of the encoded key.
    const WIDTH: u16;

    /// Stable type tag written into block headers.
    const TYPE_TAG: [u8; 16];

    /// Encode this key into the provided output buffer.
    ///
    /// # Panics
    ///
    /// Panics if `out.len()` does not equal [`Self::WIDTH`].
    fn encode_into(&self, out: &mut [u8]);

    /// Decode a key from bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the bytes do not represent a valid key.
    fn decode_from(bytes: &[u8]) -> Result<Self, DryIceError>;
}

/// Marker type for unkeyed readers and writers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoRecordKey;

/// Built-in fixed-width 8-byte key type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Bytes8Key(pub [u8; 8]);

impl From<[u8; 8]> for Bytes8Key {
    fn from(value: [u8; 8]) -> Self {
        Self(value)
    }
}

impl RecordKey for Bytes8Key {
    const WIDTH: u16 = 8;
    const TYPE_TAG: [u8; 16] = *b"dryi:bytes8:key!";

    fn encode_into(&self, out: &mut [u8]) {
        debug_assert_eq!(out.len(), usize::from(Self::WIDTH));
        out.copy_from_slice(&self.0);
    }

    fn decode_from(bytes: &[u8]) -> Result<Self, DryIceError> {
        let arr: [u8; 8] = bytes
            .try_into()
            .map_err(|_| DryIceError::InvalidRecordKeyEncoding {
                message: "invalid bytes8 key length",
            })?;
        Ok(Self(arr))
    }
}

/// Built-in fixed-width 16-byte key type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Bytes16Key(pub [u8; 16]);

impl From<[u8; 16]> for Bytes16Key {
    fn from(value: [u8; 16]) -> Self {
        Self(value)
    }
}

impl RecordKey for Bytes16Key {
    const WIDTH: u16 = 16;
    const TYPE_TAG: [u8; 16] = *b"dryi:bytes16:key";

    fn encode_into(&self, out: &mut [u8]) {
        debug_assert_eq!(out.len(), usize::from(Self::WIDTH));
        out.copy_from_slice(&self.0);
    }

    fn decode_from(bytes: &[u8]) -> Result<Self, DryIceError> {
        let arr: [u8; 16] =
            bytes
                .try_into()
                .map_err(|_| DryIceError::InvalidRecordKeyEncoding {
                    message: "invalid bytes16 key length",
                })?;
        Ok(Self(arr))
    }
}

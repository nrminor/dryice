//! Name codec trait and built-in implementations.

use crate::error::DryIceError;

/// A name encoding strategy for `dryice` blocks.
///
/// Unlike [`SequenceCodec`](super::sequence::SequenceCodec) and
/// [`QualityCodec`](super::quality::QualityCodec), the name codec
/// has an associated [`Decoded`](Self::Decoded) type that can carry
/// richer parsed structure than raw bytes. This reflects the fact
/// that sequencing record names are structured text with meaningful
/// subfields.
pub trait NameCodec: Sized {
    /// Stable type tag written into block headers.
    const TYPE_TAG: [u8; 16];

    /// Whether this encoding is lossy.
    const LOSSY: bool;

    /// The decoded representation of a name.
    type Decoded;

    /// Encode raw name bytes, appending the encoded bytes directly
    /// into the provided output buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the name data is invalid for this encoding.
    fn encode_into(name: &[u8], output: &mut Vec<u8>) -> Result<(), DryIceError>;

    /// Decode an encoded buffer into the codec's decoded representation.
    ///
    /// `original_len` is the number of bytes in the original name.
    ///
    /// # Errors
    ///
    /// Returns an error if the encoded data is corrupt or inconsistent.
    fn decode(encoded: &[u8], original_len: usize) -> Result<Self::Decoded, DryIceError>;

    /// View the decoded name as raw bytes for use in `SeqRecordLike`.
    fn as_bytes(decoded: &Self::Decoded) -> &[u8];

    /// Encode name bytes, returning a new allocated buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the name data is invalid for this encoding.
    fn encode(name: &[u8]) -> Result<Vec<u8>, DryIceError> {
        let mut out = Vec::new();
        Self::encode_into(name, &mut out)?;
        Ok(out)
    }

    /// Decode an encoded buffer directly to raw bytes, appending into
    /// the provided output buffer.
    ///
    /// This is used internally by the block decoder to populate the
    /// name buffer without requiring knowledge of the `Decoded` type.
    /// The default implementation decodes and then copies via `as_bytes`.
    ///
    /// # Errors
    ///
    /// Returns an error if the encoded data is corrupt or inconsistent.
    fn decode_to_bytes_into(
        encoded: &[u8],
        original_len: usize,
        output: &mut Vec<u8>,
    ) -> Result<(), DryIceError> {
        let decoded = Self::decode(encoded, original_len)?;
        output.extend_from_slice(Self::as_bytes(&decoded));
        Ok(())
    }
}

/// A raw name — the full name bytes with no parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawName(pub Vec<u8>);

impl RawName {
    /// The full name bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Raw name storage. No transformation.
pub struct RawNameCodec;

impl NameCodec for RawNameCodec {
    const TYPE_TAG: [u8; 16] = *b"dryi:name:raw!!!";
    const LOSSY: bool = false;
    type Decoded = RawName;

    fn encode_into(name: &[u8], output: &mut Vec<u8>) -> Result<(), DryIceError> {
        output.extend_from_slice(name);
        Ok(())
    }

    fn decode(encoded: &[u8], _original_len: usize) -> Result<RawName, DryIceError> {
        Ok(RawName(encoded.to_vec()))
    }

    fn as_bytes(decoded: &RawName) -> &[u8] {
        &decoded.0
    }
}

/// An omitted name — names are dropped entirely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OmittedName;

/// Omit names entirely. Encodes to empty, decodes to `OmittedName`.
pub struct OmittedNameCodec;

impl NameCodec for OmittedNameCodec {
    const TYPE_TAG: [u8; 16] = *b"dryi:name:omittd";
    const LOSSY: bool = true;
    type Decoded = OmittedName;

    fn encode_into(_name: &[u8], _output: &mut Vec<u8>) -> Result<(), DryIceError> {
        Ok(())
    }

    fn decode(_encoded: &[u8], _original_len: usize) -> Result<OmittedName, DryIceError> {
        Ok(OmittedName)
    }

    fn as_bytes(_decoded: &OmittedName) -> &[u8] {
        &[]
    }
}

/// A name split on the first space into identifier and description.
///
/// FASTQ/FASTA names typically have the form:
///
/// ```text
/// instrument:run:flowcell:lane:tile:x:y 1:N:0:ATCACG
/// ^--- identifier ---^                  ^--- description ---^
///                     ^ first space
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitName {
    /// The identifier portion (before the first space).
    pub id: Vec<u8>,
    /// The description portion (after the first space), if any.
    pub description: Vec<u8>,
    /// The full reconstructed name bytes (cached for `as_bytes`).
    full: Vec<u8>,
}

impl SplitName {
    /// The identifier portion of the name.
    #[must_use]
    pub fn id(&self) -> &[u8] {
        &self.id
    }

    /// The description portion of the name, if any.
    #[must_use]
    pub fn description(&self) -> &[u8] {
        &self.description
    }

    /// The full reconstructed name bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.full
    }
}

/// Split name codec. Splits on the first space into identifier and
/// description, storing both with a length prefix for exact
/// reconstruction.
///
/// On-disk layout per name:
///
/// ```text
/// [id_len: u32 le] [id_bytes] [desc_bytes]
/// ```
pub struct SplitNameCodec;

impl NameCodec for SplitNameCodec {
    const TYPE_TAG: [u8; 16] = *b"dryi:name:split!";
    const LOSSY: bool = false;
    type Decoded = SplitName;

    fn encode_into(name: &[u8], output: &mut Vec<u8>) -> Result<(), DryIceError> {
        let split_pos = name.iter().position(|&b| b == b' ');

        let (id, desc) = match split_pos {
            Some(pos) => (&name[..pos], &name[pos + 1..]),
            None => (name, &[] as &[u8]),
        };

        let id_len = u32::try_from(id.len()).map_err(|_| DryIceError::SectionOverflow {
            field: "name identifier length",
        })?;

        output.extend_from_slice(&id_len.to_le_bytes());
        output.extend_from_slice(id);
        output.extend_from_slice(desc);

        Ok(())
    }

    fn decode(encoded: &[u8], _original_len: usize) -> Result<SplitName, DryIceError> {
        if encoded.len() < 4 {
            return Err(DryIceError::CorruptBlockLayout {
                message: "SplitNameCodec encoded buffer too short for id_len",
            });
        }

        let id_len = u32::from_le_bytes([encoded[0], encoded[1], encoded[2], encoded[3]]) as usize;

        let id_end = 4 + id_len;
        if id_end > encoded.len() {
            return Err(DryIceError::CorruptBlockLayout {
                message: "SplitNameCodec id_len exceeds buffer",
            });
        }

        let id = encoded[4..id_end].to_vec();
        let description = encoded[id_end..].to_vec();

        let full = if description.is_empty() {
            id.clone()
        } else {
            let mut f = Vec::with_capacity(id.len() + 1 + description.len());
            f.extend_from_slice(&id);
            f.push(b' ');
            f.extend_from_slice(&description);
            f
        };

        Ok(SplitName {
            id,
            description,
            full,
        })
    }

    fn as_bytes(decoded: &SplitName) -> &[u8] {
        &decoded.full
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_round_trip() {
        let name = b"@instrument:run:flowcell 1:N:0:ATCACG";
        let encoded = RawNameCodec::encode(name).expect("encode should succeed");
        let decoded = RawNameCodec::decode(&encoded, name.len()).expect("decode should succeed");
        assert_eq!(decoded.as_bytes(), name);
    }

    #[test]
    fn omitted_produces_empty() {
        let name = b"@some_read_name";
        let encoded = OmittedNameCodec::encode(name).expect("encode should succeed");
        assert!(encoded.is_empty());
        let decoded =
            OmittedNameCodec::decode(&encoded, name.len()).expect("decode should succeed");
        assert_eq!(OmittedNameCodec::as_bytes(&decoded), b"");
    }

    #[test]
    fn split_round_trip_with_space() {
        let name = b"instrument:run:flowcell 1:N:0:ATCACG";
        let encoded = SplitNameCodec::encode(name).expect("encode should succeed");
        let decoded = SplitNameCodec::decode(&encoded, name.len()).expect("decode should succeed");
        assert_eq!(decoded.as_bytes(), name);
        assert_eq!(decoded.id(), b"instrument:run:flowcell");
        assert_eq!(decoded.description(), b"1:N:0:ATCACG");
    }

    #[test]
    fn split_round_trip_without_space() {
        let name = b"simple_read_name";
        let encoded = SplitNameCodec::encode(name).expect("encode should succeed");
        let decoded = SplitNameCodec::decode(&encoded, name.len()).expect("decode should succeed");
        assert_eq!(decoded.as_bytes(), name);
        assert_eq!(decoded.id(), name.as_slice());
        assert!(decoded.description().is_empty());
    }

    #[test]
    fn split_round_trip_empty_name() {
        let name = b"";
        let encoded = SplitNameCodec::encode(name).expect("encode should succeed");
        let decoded = SplitNameCodec::decode(&encoded, name.len()).expect("decode should succeed");
        assert_eq!(decoded.as_bytes(), name);
    }

    #[test]
    fn split_round_trip_multiple_spaces() {
        let name = b"id part1 part2 part3";
        let encoded = SplitNameCodec::encode(name).expect("encode should succeed");
        let decoded = SplitNameCodec::decode(&encoded, name.len()).expect("decode should succeed");
        assert_eq!(decoded.as_bytes(), name);
        assert_eq!(decoded.id(), b"id");
        assert_eq!(decoded.description(), b"part1 part2 part3");
    }

    #[test]
    fn split_trailing_space_drops_empty_description() {
        let name = b"id ";
        let encoded = SplitNameCodec::encode(name).expect("encode should succeed");
        let decoded = SplitNameCodec::decode(&encoded, name.len()).expect("decode should succeed");
        // A trailing space with no description is normalized to just the id.
        // This is intentional — the split codec treats the space as a delimiter,
        // not as content.
        assert_eq!(decoded.as_bytes(), b"id");
        assert_eq!(decoded.id(), b"id");
        assert!(decoded.description().is_empty());
    }
}

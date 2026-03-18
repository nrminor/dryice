//! Sequence codec trait and built-in implementations.

use crate::error::DryIceError;

/// A sequence encoding strategy for `dryice` blocks.
///
/// Implementors define how raw ASCII nucleotide sequences are encoded
/// for on-disk storage and decoded back. The crate provides
/// [`RawAsciiCodec`] and [`TwoBitExactCodec`] as built-in
/// implementations, but users can implement this trait for custom
/// encodings.
pub trait SequenceCodec: Sized {
    /// Stable type tag written into block headers.
    const TYPE_TAG: [u8; 16];

    /// Whether this encoding is lossy.
    const LOSSY: bool;

    /// Encode a raw ASCII nucleotide sequence into the codec's format.
    ///
    /// # Errors
    ///
    /// Returns an error if the sequence contains bytes that are invalid
    /// for this encoding.
    fn encode(sequence: &[u8]) -> Result<Vec<u8>, DryIceError>;

    /// Decode an encoded buffer back into raw ASCII nucleotide bytes.
    ///
    /// `original_len` is the number of bases in the original sequence,
    /// needed because some encodings pad or compress.
    ///
    /// # Errors
    ///
    /// Returns an error if the encoded data is corrupt or inconsistent.
    fn decode(encoded: &[u8], original_len: usize) -> Result<Vec<u8>, DryIceError>;
}

/// Raw ASCII sequence storage. No transformation — fastest possible
/// encode and decode, largest on-disk footprint.
pub struct RawAsciiCodec;

impl SequenceCodec for RawAsciiCodec {
    const TYPE_TAG: [u8; 16] = *b"dryi:seq:raw-asc";
    const LOSSY: bool = false;

    fn encode(sequence: &[u8]) -> Result<Vec<u8>, DryIceError> {
        Ok(sequence.to_vec())
    }

    fn decode(encoded: &[u8], _original_len: usize) -> Result<Vec<u8>, DryIceError> {
        Ok(encoded.to_vec())
    }
}

/// Exact 2-bit sequence encoding with sparse ambiguity sideband.
///
/// Canonical bases (A, C, G, T) are packed into 2 bits each via
/// `bitnuc` with SIMD acceleration. Non-canonical IUPAC bases are
/// stored in a sparse sideband for exact reconstruction.
///
/// On-disk layout per record:
///
/// ```text
/// [2-bit packed bases as le u64s]
/// [ambiguity_count: u32 le]
/// [positions: u32 le each]
/// [IUPAC bytes: u8 each]
/// ```
///
/// Lowercase canonical bases are normalized to uppercase during
/// encoding. Ambiguous bases preserve their original byte value.
pub struct TwoBitExactCodec;

impl SequenceCodec for TwoBitExactCodec {
    const TYPE_TAG: [u8; 16] = *b"dryi:seq:2b-exct";
    const LOSSY: bool = false;

    fn encode(sequence: &[u8]) -> Result<Vec<u8>, DryIceError> {
        if sequence.is_empty() {
            let mut out = Vec::with_capacity(4);
            out.extend_from_slice(&0u32.to_le_bytes());
            return Ok(out);
        }

        let mut canonical = Vec::with_capacity(sequence.len());
        let mut ambig_positions: Vec<u32> = Vec::new();
        let mut ambig_bytes: Vec<u8> = Vec::new();

        for (i, &base) in sequence.iter().enumerate() {
            if is_canonical(base) {
                canonical.push(base);
            } else {
                canonical.push(b'A');
                let pos = u32::try_from(i).map_err(|_| DryIceError::SectionOverflow {
                    field: "ambiguity position",
                })?;
                ambig_positions.push(pos);
                ambig_bytes.push(base);
            }
        }

        let mut packed_bases: Vec<u64> = Vec::new();
        bitnuc::twobit::encode(&canonical, &mut packed_bases).map_err(|_| {
            DryIceError::InvalidSequenceInput {
                message: "sequence contains bytes invalid for 2-bit encoding",
            }
        })?;

        let packed_byte_len = packed_bases.len() * 8;
        let ambig_count =
            u32::try_from(ambig_positions.len()).map_err(|_| DryIceError::SectionOverflow {
                field: "ambiguity count",
            })?;
        let sideband_len = 4 + (ambig_positions.len() * 4) + ambig_bytes.len();
        let total_len = packed_byte_len + sideband_len;

        let mut out = Vec::with_capacity(total_len);

        for word in &packed_bases {
            out.extend_from_slice(&word.to_le_bytes());
        }

        out.extend_from_slice(&ambig_count.to_le_bytes());
        for &pos in &ambig_positions {
            out.extend_from_slice(&pos.to_le_bytes());
        }
        out.extend_from_slice(&ambig_bytes);

        Ok(out)
    }

    fn decode(encoded: &[u8], original_len: usize) -> Result<Vec<u8>, DryIceError> {
        let packed_word_count = original_len.div_ceil(32);
        let packed_byte_len = packed_word_count * 8;

        if encoded.len() < packed_byte_len + 4 {
            return Err(DryIceError::CorruptBlockLayout {
                message: "TwoBitExact encoded buffer too short",
            });
        }

        let mut packed_words: Vec<u64> = Vec::with_capacity(packed_word_count);
        for chunk in encoded[..packed_byte_len].chunks_exact(8) {
            packed_words.push(u64::from_le_bytes([
                chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
            ]));
        }

        let mut decoded = Vec::with_capacity(original_len);
        bitnuc::twobit::decode(&packed_words, original_len, &mut decoded).map_err(|_| {
            DryIceError::CorruptBlockLayout {
                message: "failed to decode 2-bit packed sequence",
            }
        })?;

        let sideband = &encoded[packed_byte_len..];
        if sideband.len() < 4 {
            return Err(DryIceError::CorruptBlockLayout {
                message: "TwoBitExact sideband missing ambiguity count",
            });
        }

        let ambig_count =
            u32::from_le_bytes([sideband[0], sideband[1], sideband[2], sideband[3]]) as usize;

        let positions_end = 4 + ambig_count * 4;
        let iupac_end = positions_end + ambig_count;

        if sideband.len() < iupac_end {
            return Err(DryIceError::CorruptBlockLayout {
                message: "TwoBitExact sideband truncated",
            });
        }

        for i in 0..ambig_count {
            let pos_offset = 4 + i * 4;
            let pos = u32::from_le_bytes([
                sideband[pos_offset],
                sideband[pos_offset + 1],
                sideband[pos_offset + 2],
                sideband[pos_offset + 3],
            ]) as usize;

            let iupac_byte = sideband[positions_end + i];

            if pos >= decoded.len() {
                return Err(DryIceError::CorruptBlockLayout {
                    message: "TwoBitExact ambiguity position out of range",
                });
            }

            decoded[pos] = iupac_byte;
        }

        Ok(decoded)
    }
}

/// Lossy 2-bit sequence encoding that collapses all ambiguous bases to `N`.
///
/// Like [`TwoBitExactCodec`], canonical bases are packed into 2 bits
/// via `bitnuc`. However, instead of preserving the exact IUPAC symbol
/// for each ambiguous position, all non-canonical bases are replaced
/// with `N` on decode. The sideband stores only positions, not original
/// symbols.
///
/// This is explicitly lossy: `R`, `Y`, `S`, `W`, etc. all become `N`.
///
/// On-disk layout per record:
///
/// ```text
/// [2-bit packed bases as le u64s]
/// [ambiguity_count: u32 le]
/// [positions: u32 le each]
/// ```
pub struct TwoBitLossyNCodec;

impl SequenceCodec for TwoBitLossyNCodec {
    const TYPE_TAG: [u8; 16] = *b"dryi:seq:2b-losN";
    const LOSSY: bool = true;

    fn encode(sequence: &[u8]) -> Result<Vec<u8>, DryIceError> {
        if sequence.is_empty() {
            let mut out = Vec::with_capacity(4);
            out.extend_from_slice(&0u32.to_le_bytes());
            return Ok(out);
        }

        let mut canonical = Vec::with_capacity(sequence.len());
        let mut ambig_positions: Vec<u32> = Vec::new();

        for (i, &base) in sequence.iter().enumerate() {
            if is_canonical(base) {
                canonical.push(base);
            } else {
                canonical.push(b'A');
                let pos = u32::try_from(i).map_err(|_| DryIceError::SectionOverflow {
                    field: "ambiguity position",
                })?;
                ambig_positions.push(pos);
            }
        }

        let mut packed_bases: Vec<u64> = Vec::new();
        bitnuc::twobit::encode(&canonical, &mut packed_bases).map_err(|_| {
            DryIceError::InvalidSequenceInput {
                message: "sequence contains bytes invalid for 2-bit encoding",
            }
        })?;

        let packed_byte_len = packed_bases.len() * 8;
        let ambig_count =
            u32::try_from(ambig_positions.len()).map_err(|_| DryIceError::SectionOverflow {
                field: "ambiguity count",
            })?;
        let sideband_len = 4 + (ambig_positions.len() * 4);
        let total_len = packed_byte_len + sideband_len;

        let mut out = Vec::with_capacity(total_len);

        for word in &packed_bases {
            out.extend_from_slice(&word.to_le_bytes());
        }

        out.extend_from_slice(&ambig_count.to_le_bytes());
        for &pos in &ambig_positions {
            out.extend_from_slice(&pos.to_le_bytes());
        }

        Ok(out)
    }

    fn decode(encoded: &[u8], original_len: usize) -> Result<Vec<u8>, DryIceError> {
        let packed_word_count = original_len.div_ceil(32);
        let packed_byte_len = packed_word_count * 8;

        if encoded.len() < packed_byte_len + 4 {
            return Err(DryIceError::CorruptBlockLayout {
                message: "TwoBitLossyN encoded buffer too short",
            });
        }

        let mut packed_words: Vec<u64> = Vec::with_capacity(packed_word_count);
        for chunk in encoded[..packed_byte_len].chunks_exact(8) {
            packed_words.push(u64::from_le_bytes([
                chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
            ]));
        }

        let mut decoded = Vec::with_capacity(original_len);
        bitnuc::twobit::decode(&packed_words, original_len, &mut decoded).map_err(|_| {
            DryIceError::CorruptBlockLayout {
                message: "failed to decode 2-bit packed sequence",
            }
        })?;

        let sideband = &encoded[packed_byte_len..];
        if sideband.len() < 4 {
            return Err(DryIceError::CorruptBlockLayout {
                message: "TwoBitLossyN sideband missing ambiguity count",
            });
        }

        let ambig_count =
            u32::from_le_bytes([sideband[0], sideband[1], sideband[2], sideband[3]]) as usize;

        let positions_end = 4 + ambig_count * 4;
        if sideband.len() < positions_end {
            return Err(DryIceError::CorruptBlockLayout {
                message: "TwoBitLossyN sideband truncated",
            });
        }

        for i in 0..ambig_count {
            let pos_offset = 4 + i * 4;
            let pos = u32::from_le_bytes([
                sideband[pos_offset],
                sideband[pos_offset + 1],
                sideband[pos_offset + 2],
                sideband[pos_offset + 3],
            ]) as usize;

            if pos >= decoded.len() {
                return Err(DryIceError::CorruptBlockLayout {
                    message: "TwoBitLossyN ambiguity position out of range",
                });
            }

            decoded[pos] = b'N';
        }

        Ok(decoded)
    }
}

fn is_canonical(base: u8) -> bool {
    matches!(base, b'A' | b'a' | b'C' | b'c' | b'G' | b'g' | b'T' | b't')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_ascii_round_trip() {
        let seq = b"ACGTACGT";
        let encoded = RawAsciiCodec::encode(seq).expect("encode should succeed");
        let decoded = RawAsciiCodec::decode(&encoded, seq.len()).expect("decode should succeed");
        assert_eq!(&decoded, seq);
    }

    #[test]
    fn two_bit_exact_round_trip_canonical_only() {
        let seq = b"ACGTACGT";
        let encoded = TwoBitExactCodec::encode(seq).expect("encode should succeed");
        let decoded = TwoBitExactCodec::decode(&encoded, seq.len()).expect("decode should succeed");
        assert_eq!(&decoded, seq);
    }

    #[test]
    fn two_bit_exact_round_trip_with_ambiguity() {
        let seq = b"ACNGTRYACGT";
        let encoded = TwoBitExactCodec::encode(seq).expect("encode should succeed");
        let decoded = TwoBitExactCodec::decode(&encoded, seq.len()).expect("decode should succeed");
        assert_eq!(&decoded, seq);
    }

    #[test]
    fn two_bit_exact_round_trip_all_ambiguous() {
        let seq = b"NNNNNN";
        let encoded = TwoBitExactCodec::encode(seq).expect("encode should succeed");
        let decoded = TwoBitExactCodec::decode(&encoded, seq.len()).expect("decode should succeed");
        assert_eq!(&decoded, seq);
    }

    #[test]
    fn two_bit_exact_round_trip_single_base() {
        let seq = b"G";
        let encoded = TwoBitExactCodec::encode(seq).expect("encode should succeed");
        let decoded = TwoBitExactCodec::decode(&encoded, seq.len()).expect("decode should succeed");
        assert_eq!(&decoded, seq);
    }

    #[test]
    fn two_bit_exact_round_trip_non_multiple_of_32() {
        let seq = b"ACGTACGTACGTACGTACGTACGTACGTACGTACG";
        assert_eq!(seq.len(), 35);
        let encoded = TwoBitExactCodec::encode(seq).expect("encode should succeed");
        let decoded = TwoBitExactCodec::decode(&encoded, seq.len()).expect("decode should succeed");
        assert_eq!(&decoded, seq);
    }

    #[test]
    fn two_bit_exact_round_trip_empty() {
        let seq = b"";
        let encoded = TwoBitExactCodec::encode(seq).expect("encode should succeed");
        let decoded = TwoBitExactCodec::decode(&encoded, seq.len()).expect("decode should succeed");
        assert_eq!(&decoded, seq);
    }

    #[test]
    fn two_bit_exact_lowercase_normalizes_to_uppercase() {
        let seq = b"acgtNacgt";
        let encoded = TwoBitExactCodec::encode(seq).expect("encode should succeed");
        let decoded = TwoBitExactCodec::decode(&encoded, seq.len()).expect("decode should succeed");
        assert_eq!(decoded, b"ACGTNACGT");
    }

    #[test]
    fn two_bit_lossy_n_collapses_ambiguity_to_n() {
        let seq = b"ACNGTRYACGT";
        let encoded = TwoBitLossyNCodec::encode(seq).expect("encode should succeed");
        let decoded =
            TwoBitLossyNCodec::decode(&encoded, seq.len()).expect("decode should succeed");
        assert_eq!(decoded, b"ACNGTNNACGT");
    }

    #[test]
    fn two_bit_lossy_n_canonical_only() {
        let seq = b"ACGTACGT";
        let encoded = TwoBitLossyNCodec::encode(seq).expect("encode should succeed");
        let decoded =
            TwoBitLossyNCodec::decode(&encoded, seq.len()).expect("decode should succeed");
        assert_eq!(&decoded, seq);
    }

    #[test]
    fn two_bit_lossy_n_all_ambiguous() {
        let seq = b"NRYSW";
        let encoded = TwoBitLossyNCodec::encode(seq).expect("encode should succeed");
        let decoded =
            TwoBitLossyNCodec::decode(&encoded, seq.len()).expect("decode should succeed");
        assert_eq!(decoded, b"NNNNN");
    }

    #[test]
    fn two_bit_lossy_n_is_more_compact_than_exact() {
        let seq = b"ACNGTRYACGT";
        let exact = TwoBitExactCodec::encode(seq).expect("exact encode");
        let lossy = TwoBitLossyNCodec::encode(seq).expect("lossy encode");
        assert!(
            lossy.len() < exact.len(),
            "lossy should be more compact: lossy={}, exact={}",
            lossy.len(),
            exact.len()
        );
    }
}

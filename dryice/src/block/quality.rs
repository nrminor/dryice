//! Quality codec trait and built-in implementations.

use crate::error::DryIceError;

/// A quality score encoding strategy for `dryice` blocks.
///
/// Implementors define how raw quality score bytes are encoded for
/// on-disk storage and decoded back. The crate provides
/// [`RawQualityCodec`] and [`BinnedQualityCodec`] as built-in
/// implementations, but users can implement this trait for custom
/// encodings.
pub trait QualityCodec: Sized {
    /// Stable type tag written into block headers.
    const TYPE_TAG: [u8; 16];

    /// Whether this encoding is lossy.
    const LOSSY: bool;

    /// Encode raw quality score bytes into the codec's format.
    ///
    /// # Errors
    ///
    /// Returns an error if the quality data is invalid for this encoding.
    fn encode(quality: &[u8]) -> Result<Vec<u8>, DryIceError>;

    /// Decode an encoded buffer back into raw quality score bytes.
    ///
    /// `original_len` is the number of quality scores in the original
    /// record, needed because some encodings may compress.
    ///
    /// # Errors
    ///
    /// Returns an error if the encoded data is corrupt or inconsistent.
    fn decode(encoded: &[u8], original_len: usize) -> Result<Vec<u8>, DryIceError>;
}

/// Raw quality score storage. No transformation.
pub struct RawQualityCodec;

impl QualityCodec for RawQualityCodec {
    const TYPE_TAG: [u8; 16] = *b"dryi:qual:raw!!!";
    const LOSSY: bool = false;

    fn encode(quality: &[u8]) -> Result<Vec<u8>, DryIceError> {
        Ok(quality.to_vec())
    }

    fn decode(encoded: &[u8], _original_len: usize) -> Result<Vec<u8>, DryIceError> {
        Ok(encoded.to_vec())
    }
}

/// Illumina-style 8-level quality score binning.
///
/// This is an explicitly lossy encoding that maps Phred quality scores
/// into 8 bins, reducing entropy for better downstream compression
/// while preserving the most important quality distinctions.
///
/// Bin boundaries and representative values:
///
/// ```text
/// Phred  0-1   → 0
/// Phred  2-9   → 6
/// Phred 10-19  → 15
/// Phred 20-24  → 22
/// Phred 25-29  → 27
/// Phred 30-34  → 33
/// Phred 35-39  → 37
/// Phred 40+    → 40
/// ```
///
/// Quality bytes are assumed to be Phred+33 encoded (standard Sanger/Illumina
/// 1.8+ encoding). The binned output is also Phred+33 encoded.
pub struct BinnedQualityCodec;

const PHRED_OFFSET: u8 = 33;

fn bin_phred(phred: u8) -> u8 {
    match phred {
        0..=1 => 0,
        2..=9 => 6,
        10..=19 => 15,
        20..=24 => 22,
        25..=29 => 27,
        30..=34 => 33,
        35..=39 => 37,
        _ => 40,
    }
}

impl QualityCodec for BinnedQualityCodec {
    const TYPE_TAG: [u8; 16] = *b"dryi:qual:binned";
    const LOSSY: bool = true;

    fn encode(quality: &[u8]) -> Result<Vec<u8>, DryIceError> {
        Ok(quality
            .iter()
            .map(|&q| {
                let phred = q.saturating_sub(PHRED_OFFSET);
                bin_phred(phred) + PHRED_OFFSET
            })
            .collect())
    }

    fn decode(encoded: &[u8], _original_len: usize) -> Result<Vec<u8>, DryIceError> {
        Ok(encoded.to_vec())
    }
}

/// An omitted quality codec that produces and expects empty quality sections.
pub struct OmittedQualityCodec;

impl QualityCodec for OmittedQualityCodec {
    const TYPE_TAG: [u8; 16] = *b"dryi:qual:omittd";
    const LOSSY: bool = true;

    fn encode(_quality: &[u8]) -> Result<Vec<u8>, DryIceError> {
        Ok(Vec::new())
    }

    fn decode(_encoded: &[u8], _original_len: usize) -> Result<Vec<u8>, DryIceError> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_round_trip() {
        let qual = b"!!!!####";
        let encoded = RawQualityCodec::encode(qual).expect("encode should succeed");
        let decoded = RawQualityCodec::decode(&encoded, qual.len()).expect("decode should succeed");
        assert_eq!(&decoded, qual);
    }

    #[test]
    fn binned_produces_valid_phred33() {
        let qual: Vec<u8> = (33..=73).collect();
        let encoded = BinnedQualityCodec::encode(&qual).expect("encode should succeed");
        for &q in &encoded {
            assert!(
                q >= PHRED_OFFSET,
                "binned quality should be >= Phred+33 offset"
            );
        }
    }

    #[test]
    fn binned_is_idempotent() {
        let qual: Vec<u8> = (33..=73).collect();
        let once = BinnedQualityCodec::encode(&qual).expect("first encode");
        let twice = BinnedQualityCodec::encode(&once).expect("second encode");
        assert_eq!(once, twice, "binning should be idempotent");
    }

    #[test]
    fn binned_preserves_length() {
        let qual = b"!!!!!!!!!!!";
        let encoded = BinnedQualityCodec::encode(qual).expect("encode should succeed");
        assert_eq!(encoded.len(), qual.len());
    }

    #[test]
    fn binned_high_quality_bins_correctly() {
        let q40 = vec![40 + PHRED_OFFSET];
        let encoded = BinnedQualityCodec::encode(&q40).expect("encode should succeed");
        assert_eq!(encoded[0], 40 + PHRED_OFFSET);
    }

    #[test]
    fn omitted_produces_empty() {
        let qual = b"!!!!####";
        let encoded = OmittedQualityCodec::encode(qual).expect("encode should succeed");
        assert!(encoded.is_empty());
    }
}

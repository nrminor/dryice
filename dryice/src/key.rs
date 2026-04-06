//! Record-key types and traits.

use crate::error::DryIceError;
use simd_minimizers::packed_seq::{PackedSeqVec, SeqVec};

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

/// A kmer-derived fixed-width record key.
///
/// `KmerKey` is intentionally a thin marker layer over [`RecordKey`]. It
/// carries the compile-time kmer length while leaving storage concerns to the
/// underlying record-key contract.
///
/// The built-in kmer key families in `dryice` all use packed canonical
/// representations by default. A concrete key type therefore tells you both:
///
/// - how the value was selected from the sequence (prefix or minimizer)
/// - how wide the packed representation is (currently 64 bits)
///
/// Kmer selection constructors return `Result<Option<Self>, DryIceError>`:
///
/// - `Ok(Some(key))` means a key was successfully derived
/// - `Ok(None)` means the sequence simply cannot yield a key for this family
///   (for example because it is too short or contains ambiguous bases)
/// - `Err(...)` is reserved for unexpected failures
pub trait KmerKey: RecordKey {
    /// Kmer length used by this key family.
    const K: u8;
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

/// Prefix-selected packed canonical kmer key stored in 64 bits.
///
/// `PrefixKmer64<K>` stores the canonical packed representation of the first
/// `K` DNA bases of a sequence. Canonical here means the minimum of the forward
/// kmer and its reverse complement, so reverse-complement sequences yield the
/// same key.
///
/// This family is intended as the simplest built-in kmer-as-key selector and
/// does not depend on any external minimizer backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PrefixKmer64<const K: u8>(pub u64);

impl<const K: u8> KmerKey for PrefixKmer64<K> {
    const K: u8 = K;
}

impl<const K: u8> RecordKey for PrefixKmer64<K> {
    const WIDTH: u16 = 8;
    const TYPE_TAG: [u8; 16] = *b"dryi:kmer:pref64";

    fn encode_into(&self, out: &mut [u8]) {
        debug_assert_eq!(out.len(), usize::from(Self::WIDTH));
        out.copy_from_slice(&self.0.to_le_bytes());
    }

    fn decode_from(bytes: &[u8]) -> Result<Self, DryIceError> {
        let arr: [u8; 8] = bytes
            .try_into()
            .map_err(|_| DryIceError::InvalidRecordKeyEncoding {
                message: "invalid prefix kmer64 key length",
            })?;
        Ok(Self(u64::from_le_bytes(arr)))
    }
}

impl<const K: u8> PrefixKmer64<K> {
    const ASSERT_VALID: () = {
        assert!(K > 0, "PrefixKmer64 requires K > 0");
        assert!(K <= 32, "PrefixKmer64 requires K <= 32");
    };

    /// Derive a prefix-selected canonical kmer key from a sequence.
    ///
    /// # Errors
    ///
    /// Returns an error only for unexpected internal failures. Expected no-key
    /// outcomes such as short or ambiguous sequences return `Ok(None)`.
    ///
    /// The constructor rejects ambiguous bases by returning `Ok(None)` because a
    /// packed canonical prefix key is only defined on unambiguous `A/C/G/T`
    /// sequences.
    pub fn try_from_sequence(seq: &[u8]) -> Result<Option<Self>, DryIceError> {
        let () = Self::ASSERT_VALID;

        if seq.len() < usize::from(K) {
            return Ok(None);
        }

        let prefix = &seq[..usize::from(K)];
        let mut forward = 0u64;
        let mut revcomp = 0u64;

        for &base in prefix {
            let bits = match base {
                b'A' | b'a' => 0u64,
                b'C' | b'c' => 1u64,
                b'G' | b'g' => 2u64,
                b'T' | b't' => 3u64,
                _ => return Ok(None),
            };
            forward = (forward << 2) | bits;
        }

        for &base in prefix.iter().rev() {
            let bits = match base {
                b'A' | b'a' => 0u64,
                b'C' | b'c' => 1u64,
                b'G' | b'g' => 2u64,
                b'T' | b't' => 3u64,
                _ => return Ok(None),
            };
            revcomp = (revcomp << 2) | (3 - bits);
        }

        Ok(Some(Self(forward.min(revcomp))))
    }
}

/// Minimizer-selected packed canonical kmer key stored in 64 bits.
///
/// `Minimizer64<K, W>` stores one canonical `K`-mer chosen from a longer
/// sequence using minimizer selection over a window of `W` consecutive `K`-mers.
/// The effective sequence span examined by the selector is therefore
/// `K + W - 1` bases.
///
/// When multiple minimizer candidates are produced for a sequence, `dryice`
/// reduces them to a single record key by taking the minimum canonical packed
/// value. This keeps the result deterministic and stable under
/// reverse-complement transforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Minimizer64<const K: u8, const W: u8>(pub u64);

impl<const K: u8, const W: u8> KmerKey for Minimizer64<K, W> {
    const K: u8 = K;
}

impl<const K: u8, const W: u8> RecordKey for Minimizer64<K, W> {
    const WIDTH: u16 = 8;
    const TYPE_TAG: [u8; 16] = *b"dryi:kmer:mini64";

    fn encode_into(&self, out: &mut [u8]) {
        debug_assert_eq!(out.len(), usize::from(Self::WIDTH));
        out.copy_from_slice(&self.0.to_le_bytes());
    }

    fn decode_from(bytes: &[u8]) -> Result<Self, DryIceError> {
        let arr: [u8; 8] = bytes
            .try_into()
            .map_err(|_| DryIceError::InvalidRecordKeyEncoding {
                message: "invalid minimizer64 key length",
            })?;
        Ok(Self(u64::from_le_bytes(arr)))
    }
}

impl<const K: u8, const W: u8> Minimizer64<K, W> {
    const ASSERT_VALID: () = {
        assert!(K > 0, "Minimizer64 requires K > 0");
        assert!(K <= 32, "Minimizer64 requires K <= 32");
        assert!(W > 0, "Minimizer64 requires W > 0");
    };

    /// Derive a minimizer-selected canonical kmer key from a sequence.
    ///
    /// # Errors
    ///
    /// Returns an error only for unexpected internal failures. Expected no-key
    /// outcomes such as short or ambiguous sequences return `Ok(None)`.
    ///
    /// This constructor currently uses `simd-minimizers` internally for
    /// canonical minimizer discovery, but `dryice` owns the public reduction
    /// semantics: one key per record, chosen as the minimum selected canonical
    /// packed value.
    pub fn try_from_sequence(seq: &[u8]) -> Result<Option<Self>, DryIceError> {
        let () = Self::ASSERT_VALID;

        let l = usize::from(K) + usize::from(W) - 1;
        if seq.len() < l {
            return Ok(None);
        }
        if !seq
            .iter()
            .all(|base| matches!(base, b'A' | b'a' | b'C' | b'c' | b'G' | b'g' | b'T' | b't'))
        {
            return Ok(None);
        }

        let packed = PackedSeqVec::from_ascii(seq);
        let mut positions = Vec::new();
        let values: Vec<u64> =
            simd_minimizers::canonical_minimizers(usize::from(K), usize::from(W))
                .run(packed.as_slice(), &mut positions)
                .values_u64()
                .collect();

        Ok(values.into_iter().min().map(Self))
    }
}

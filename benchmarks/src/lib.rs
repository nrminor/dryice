//! Shared utilities for dryice benchmarks.

use dryice::{Bytes8Key, SeqRecord};

/// Generate a batch of realistic Illumina-like sequencing records.
///
/// Each record has:
/// - a name resembling Illumina instrument output
/// - a 150bp sequence with ~1% ambiguity rate
/// - Phred+33 quality scores with a realistic distribution
///
/// # Panics
///
/// Panics if a synthetic record cannot be constructed (should not
/// happen with the hardcoded generation logic).
#[must_use]
pub fn generate_records(count: usize) -> Vec<SeqRecord> {
    let bases = [b'A', b'C', b'G', b'T'];

    (0..count)
        .map(|i| {
            let name = format!(
                "INSTRUMENT:RUN:FLOWCELL:1:1101:{}:{} 1:N:0:ATCACG",
                1000 + i / 100,
                2000 + i % 100
            )
            .into_bytes();

            let seq: Vec<u8> = (0..150)
                .map(|j| {
                    if (i * 31 + j * 17) % 100 == 0 {
                        b'N'
                    } else {
                        bases[(i * 7 + j * 13) % 4]
                    }
                })
                .collect();

            let qual: Vec<u8> = (0..150)
                .map(|j| {
                    let phred =
                        u8::try_from((i * 3 + j * 11) % 35 + 5).expect("phred value fits in u8");
                    phred + 33
                })
                .collect();

            SeqRecord::new(name, seq, qual).expect("valid synthetic record")
        })
        .collect()
}

/// Compute a simple 8-byte sort key from a sequence.
#[must_use]
pub fn compute_sort_key(sequence: &[u8]) -> Bytes8Key {
    let mut key = [0u8; 8];
    for (i, &b) in sequence.iter().take(8).enumerate() {
        key[i] = b;
    }
    Bytes8Key(key)
}

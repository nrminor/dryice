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

/// Total payload size of a record set in bytes (name + sequence + quality).
#[must_use]
pub fn payload_size(records: &[SeqRecord]) -> usize {
    records
        .iter()
        .map(|r| r.name().len() + r.sequence().len() + r.quality().len())
        .sum()
}

/// Write records as FASTQ into a byte buffer.
pub fn write_fastq(records: &[SeqRecord], buf: &mut Vec<u8>) {
    for record in records {
        buf.push(b'@');
        buf.extend_from_slice(record.name());
        buf.push(b'\n');
        buf.extend_from_slice(record.sequence());
        buf.extend_from_slice(b"\n+\n");
        buf.extend_from_slice(record.quality());
        buf.push(b'\n');
    }
}

/// Read FASTQ records from a byte buffer, calling `f` for each record's
/// (name, sequence, quality) slices.
///
/// This is a minimal line-based scanner, not a production FASTQ parser.
/// It exists to benchmark format overhead without testing a real parser.
pub fn read_fastq<F>(buf: &[u8], mut f: F)
where
    F: FnMut(&[u8], &[u8], &[u8]),
{
    let mut pos = 0;
    while pos < buf.len() {
        let name_start = pos + 1;
        let name_end = memchr(b'\n', &buf[name_start..]).map_or(buf.len(), |i| name_start + i);
        let seq_start = name_end + 1;
        let seq_end = memchr(b'\n', &buf[seq_start..]).map_or(buf.len(), |i| seq_start + i);
        let plus_end = memchr(b'\n', &buf[seq_end + 1..]).map_or(buf.len(), |i| seq_end + 1 + i);
        let qual_start = plus_end + 1;
        let qual_end = memchr(b'\n', &buf[qual_start..]).map_or(buf.len(), |i| qual_start + i);

        f(
            &buf[name_start..name_end],
            &buf[seq_start..seq_end],
            &buf[qual_start..qual_end],
        );

        pos = qual_end + 1;
    }
}

/// Write records as raw binary: `[name_len: u32 le] [name] [seq_len: u32 le] [seq] [qual]`.
///
/// # Panics
///
/// Panics if name or sequence length exceeds `u32::MAX`.
pub fn write_raw_binary(records: &[SeqRecord], buf: &mut Vec<u8>) {
    for record in records {
        let name = record.name();
        let seq = record.sequence();
        let qual = record.quality();
        buf.extend_from_slice(
            &u32::try_from(name.len())
                .expect("name length fits in u32")
                .to_le_bytes(),
        );
        buf.extend_from_slice(name);
        buf.extend_from_slice(
            &u32::try_from(seq.len())
                .expect("sequence length fits in u32")
                .to_le_bytes(),
        );
        buf.extend_from_slice(seq);
        buf.extend_from_slice(qual);
    }
}

/// Read raw binary records, calling `f` for each record's
/// (name, sequence, quality) slices.
pub fn read_raw_binary<F>(buf: &[u8], mut f: F)
where
    F: FnMut(&[u8], &[u8], &[u8]),
{
    let mut pos = 0;
    while pos < buf.len() {
        let name_len =
            u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]) as usize;
        pos += 4;
        let name = &buf[pos..pos + name_len];
        pos += name_len;

        let seq_len =
            u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]) as usize;
        pos += 4;
        let seq = &buf[pos..pos + seq_len];
        pos += seq_len;

        let qual = &buf[pos..pos + seq_len];
        pos += seq_len;

        f(name, seq, qual);
    }
}

fn memchr(needle: u8, haystack: &[u8]) -> Option<usize> {
    haystack.iter().position(|&b| b == needle)
}

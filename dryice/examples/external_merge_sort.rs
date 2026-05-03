//! External k-way merge sort using record keys.
//!
//! This example demonstrates the flagship use case for `dryice`:
//! sorting a collection of sequencing records that is too large to
//! fit in memory by spilling sorted runs to temporary `dryice` files
//! and then merging them using precomputed sort keys.
//!
//! The merge phase compares only the 8-byte record keys — it never
//! touches the full sequence or quality payloads during comparison.
//! Records are piped from the winning reader to the output writer
//! with zero per-record allocation.
//!
//! Run with: `cargo run --example external_merge_sort`

use std::{cmp::Ordering, collections::BinaryHeap};

use dryice::{
    Bytes8Key, DryIceError, DryIceReader, DryIceWriter, SeqRecord, SeqRecordLike, TempDryIceFile,
};

/// Compute a simple sort key from a sequence by hashing the first
/// 8 bytes. In a real application, this would be a minimizer hash,
/// canonical k-mer, or other sequence-derived ordering key.
fn compute_sort_key(sequence: &[u8]) -> Bytes8Key {
    let mut key = [0u8; 8];
    for (i, &b) in sequence.iter().take(8).enumerate() {
        key[i] = b;
    }
    Bytes8Key(key)
}

/// A heap entry that tracks which sorted run a record came from.
struct MergeEntry {
    key: Bytes8Key,
    run_index: usize,
}

impl PartialEq for MergeEntry {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for MergeEntry {}

impl PartialOrd for MergeEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MergeEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap behavior with BinaryHeap.
        other.key.cmp(&self.key)
    }
}

fn generate_records(total_records: usize) -> Vec<SeqRecord> {
    (0..total_records)
        .map(|i: usize| {
            let name = format!("read_{i:04}").into_bytes();
            let bases = [b'A', b'C', b'G', b'T'];
            let seq: Vec<u8> = (0..80)
                .map(|j: usize| bases[(i * 7 + j * 13) % 4])
                .collect();
            let qual = vec![b'I'; seq.len()];
            SeqRecord::new(name, seq, qual).expect("valid record")
        })
        .collect()
}

fn create_sorted_runs(
    records: &[SeqRecord],
    chunk_size: usize,
) -> Result<Vec<TempDryIceFile>, DryIceError> {
    let mut sorted_runs: Vec<TempDryIceFile> = Vec::new();

    for chunk in records.chunks(chunk_size) {
        let mut keyed: Vec<(Bytes8Key, &SeqRecord)> = chunk
            .iter()
            .map(|r| (compute_sort_key(r.sequence()), r))
            .collect();

        keyed.sort_by_key(|(key, _)| *key);

        let run = TempDryIceFile::new()?;
        {
            let file = run.open()?;
            let mut writer = DryIceWriter::builder()
                .inner(file)
                .bytes8_key()
                .target_block_records(25)
                .build();

            for (key, record) in &keyed {
                writer.write_record_with_key(*record, key)?;
            }
            writer.finish()?;
        }

        sorted_runs.push(run);
    }

    Ok(sorted_runs)
}

fn sorted_run_bytes(sorted_runs: &[TempDryIceFile]) -> Result<u64, DryIceError> {
    let mut total = 0u64;
    for run in sorted_runs {
        total += run.path().metadata()?.len();
    }
    Ok(total)
}

fn merge_sorted_runs(sorted_runs: &[TempDryIceFile]) -> Result<Vec<u8>, DryIceError> {
    let mut readers: Vec<_> = sorted_runs
        .iter()
        .map(|run| {
            let file = run.open()?;
            DryIceReader::with_bytes8_key(file)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut heap = BinaryHeap::new();
    for (i, reader) in readers.iter_mut().enumerate() {
        if reader.next_record()? {
            let key = reader.record_key()?;
            heap.push(MergeEntry { key, run_index: i });
        }
    }

    let mut output_buf = Vec::new();
    let mut output_writer = DryIceWriter::builder()
        .inner(&mut output_buf)
        .bytes8_key()
        .build();

    while let Some(entry) = heap.pop() {
        let reader = &readers[entry.run_index];
        let record = SeqRecord::from_slices(reader.name(), reader.sequence(), reader.quality())?;
        output_writer.write_record_with_key(&record, &entry.key)?;

        let reader = &mut readers[entry.run_index];
        if reader.next_record()? {
            let key = reader.record_key()?;
            heap.push(MergeEntry {
                key,
                run_index: entry.run_index,
            });
        }
    }
    output_writer.finish()?;

    Ok(output_buf)
}

fn verify_sorted_output(output_buf: &[u8]) -> Result<usize, DryIceError> {
    let mut verify_reader = DryIceReader::with_bytes8_key(output_buf)?;
    let mut prev_key: Option<Bytes8Key> = None;
    let mut verified = 0;
    while verify_reader.next_record()? {
        let key = verify_reader.record_key()?;
        if let Some(prev) = &prev_key {
            assert!(key >= *prev, "output is not sorted at record {verified}");
        }
        prev_key = Some(key);
        verified += 1;
    }

    Ok(verified)
}

fn main() -> Result<(), DryIceError> {
    let total_records = 200;
    let chunk_size = 50;
    let all_records = generate_records(total_records);

    println!(
        "Generated {} records, sorting in chunks of {}",
        all_records.len(),
        chunk_size
    );

    // Phase 1: create sorted runs. In a real application, each chunk would be
    // read from a FASTQ file until RAM is full, sorted in memory, and spilled
    // to an owned temporary file.
    let sorted_runs = create_sorted_runs(&all_records, chunk_size)?;

    println!(
        "Created {} sorted runs ({} bytes total)",
        sorted_runs.len(),
        sorted_run_bytes(&sorted_runs)?
    );

    // Phase 2: k-way merge using keys only.
    //
    // Open a reader for each sorted run and seed a min-heap with
    // the first record's key from each run. The merge loop pops
    // the smallest key, emits the corresponding record, and
    // advances that reader.
    let output_buf = merge_sorted_runs(&sorted_runs)?;

    println!(
        "Merged {} records into {} bytes",
        total_records,
        output_buf.len()
    );

    // Verify the output is sorted by key.
    let verified = verify_sorted_output(&output_buf)?;

    println!("Verified {verified} records are in sorted order");
    assert_eq!(verified, total_records);

    for run in sorted_runs {
        run.cleanup()?;
    }

    Ok(())
}

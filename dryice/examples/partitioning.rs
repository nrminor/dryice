//! Temporary partitioning of records into buckets.
//!
//! This example demonstrates using dryice to partition records into
//! separate temporary buffers based on some derived criterion (here,
//! the first base of the sequence). Each partition gets its own
//! writer, and records can be reloaded from any partition later.
//!
//! Run with: `cargo run --example partitioning`

use dryice::{DryIceReader, DryIceWriter, SeqRecord};

fn main() -> Result<(), dryice::DryIceError> {
    let records = vec![
        SeqRecord::new(b"r1".to_vec(), b"ACGTACGT".to_vec(), b"!!!!!!!!".to_vec())?,
        SeqRecord::new(b"r2".to_vec(), b"TGCATGCA".to_vec(), b"########".to_vec())?,
        SeqRecord::new(b"r3".to_vec(), b"ACGTTTTT".to_vec(), b"!!!!!!!!".to_vec())?,
        SeqRecord::new(b"r4".to_vec(), b"GCGCGCGC".to_vec(), b"$$$$$$$$".to_vec())?,
        SeqRecord::new(b"r5".to_vec(), b"CCCCAAAA".to_vec(), b"%%%%%%%%".to_vec())?,
        SeqRecord::new(b"r6".to_vec(), b"TTTTTGGG".to_vec(), b"&&&&&&&&".to_vec())?,
    ];

    // Partition into 4 buckets by first base.
    let mut buckets: Vec<Vec<u8>> = vec![Vec::new(); 4];
    let mut writers: Vec<DryIceWriter<&mut Vec<u8>>> = buckets
        .iter_mut()
        .map(|buf| DryIceWriter::builder().inner(buf).build())
        .collect();

    for record in &records {
        let bucket = match record.sequence().first() {
            Some(b'C') => 1,
            Some(b'G') => 2,
            Some(b'T') => 3,
            _ => 0,
        };
        writers[bucket].write_record(record)?;
    }

    for writer in writers {
        writer.finish()?;
    }

    // Read back each partition.
    let labels = ["A-bucket", "C-bucket", "G-bucket", "T-bucket"];
    for (i, buf) in buckets.iter().enumerate() {
        if buf.is_empty() {
            println!("{}: empty", labels[i]);
            continue;
        }

        let mut reader = DryIceReader::new(buf.as_slice())?;
        let mut count = 0;
        while reader.next_record()? {
            count += 1;
        }
        println!("{}: {count} records ({} bytes)", labels[i], buf.len());
    }

    Ok(())
}

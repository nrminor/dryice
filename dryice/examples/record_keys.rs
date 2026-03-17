//! Record keys for merge-style access.
//!
//! This example demonstrates using record keys to store precomputed
//! sort keys alongside records. In a real k-way merge sort, the merge
//! heap would compare keys without touching the full record payloads.
//!
//! Run with: `cargo run --example record_keys`

use dryice::{Bytes8Key, DryIceReader, DryIceWriter, SeqRecord, SeqRecordLike};

fn main() -> Result<(), dryice::DryIceError> {
    let records = [
        SeqRecord::new(b"r1".to_vec(), b"AAAA".to_vec(), b"!!!!".to_vec())?,
        SeqRecord::new(b"r2".to_vec(), b"CCCC".to_vec(), b"!!!!".to_vec())?,
        SeqRecord::new(b"r3".to_vec(), b"GGGG".to_vec(), b"!!!!".to_vec())?,
        SeqRecord::new(b"r4".to_vec(), b"TTTT".to_vec(), b"!!!!".to_vec())?,
    ];

    // Compute sort keys (here, just a hash of the sequence).
    let keys: Vec<Bytes8Key> = records
        .iter()
        .map(|r| {
            let mut key = [0u8; 8];
            for (i, &b) in r.sequence().iter().enumerate() {
                key[i % 8] ^= b;
            }
            Bytes8Key(key)
        })
        .collect();

    // Write records with keys.
    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder().inner(&mut buf).bytes8_key().build();

    for (record, key) in records.iter().zip(keys.iter()) {
        writer.write_record_with_key(record, key)?;
    }
    writer.finish()?;

    println!("Wrote {} records with 8-byte keys", records.len());

    // Read back and access keys.
    let mut reader = DryIceReader::with_bytes8_key(buf.as_slice())?;
    while reader.next_record()? {
        let key = reader.record_key()?;
        let name = std::str::from_utf8(reader.name()).unwrap_or("<non-utf8>");
        println!("  {name}: key={:?}", key.0);
    }

    Ok(())
}

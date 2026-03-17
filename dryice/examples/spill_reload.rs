//! Spill/reload pattern for external sorting.
//!
//! This example demonstrates the core use case for dryice: spilling
//! a batch of sequencing records to a temporary buffer (standing in
//! for a temp file), then reloading them. In a real external sort,
//! you would spill sorted runs and then merge them back.
//!
//! Run with: `cargo run --example spill_reload`

use dryice::{DryIceReader, DryIceWriter, SeqRecord};

fn main() -> Result<(), dryice::DryIceError> {
    let records: Vec<SeqRecord> = (0..100)
        .map(|i| {
            let name = format!("read_{i:04}").into_bytes();
            let seq = format!("ACGT{}", "ACGT".repeat(i % 10)).into_bytes();
            let qual =
                vec![b'!' + u8::try_from(i % 40).expect("quality offset fits in u8"); seq.len()];
            SeqRecord::new(name, seq, qual).expect("valid record")
        })
        .collect();

    println!("Generated {} records", records.len());

    // Spill to a buffer (in practice, this would be a temp file).
    let mut spill_buf = Vec::new();
    let mut writer = DryIceWriter::builder()
        .inner(&mut spill_buf)
        .target_block_records(25)
        .build();

    for record in &records {
        writer.write_record(record)?;
    }
    writer.finish()?;

    println!(
        "Spilled {} bytes ({} bytes/record avg)",
        spill_buf.len(),
        spill_buf.len() / records.len()
    );

    // Reload from the buffer.
    let mut reader = DryIceReader::new(spill_buf.as_slice())?;
    let mut count = 0;
    while reader.next_record()? {
        count += 1;
    }

    println!("Reloaded {count} records (zero-copy)");

    // Or reload into owned records if needed.
    let reader = DryIceReader::new(spill_buf.as_slice())?;
    let reloaded: Vec<SeqRecord> = reader.into_records().collect::<Result<Vec<_>, _>>()?;

    assert_eq!(reloaded.len(), records.len());
    for (orig, back) in records.iter().zip(reloaded.iter()) {
        assert_eq!(orig.name(), back.name());
        assert_eq!(orig.sequence(), back.sequence());
        assert_eq!(orig.quality(), back.quality());
    }

    println!("Verified all records match");
    Ok(())
}

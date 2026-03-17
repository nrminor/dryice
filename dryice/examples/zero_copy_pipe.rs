//! Zero-copy reader-to-writer piping.
//!
//! This example demonstrates piping records from one dryice file to
//! another without any per-record heap allocation. The reader
//! implements `SeqRecordLike`, so it can be passed directly to the
//! writer's `write_record` method.
//!
//! Run with: `cargo run --example zero_copy_pipe`

use dryice::{DryIceReader, DryIceWriter, SeqRecord};

fn main() -> Result<(), dryice::DryIceError> {
    let records: Vec<SeqRecord> = (0..50)
        .map(|i| {
            SeqRecord::new(
                format!("read_{i}").into_bytes(),
                b"ACGTACGTACGTACGT".to_vec(),
                b"!!!!############".to_vec(),
            )
            .expect("valid record")
        })
        .collect();

    // Write the source file.
    let mut source_buf = Vec::new();
    let mut writer = DryIceWriter::builder()
        .inner(&mut source_buf)
        .target_block_records(10)
        .build();
    for record in &records {
        writer.write_record(record)?;
    }
    writer.finish()?;

    println!(
        "Source: {} records, {} bytes",
        records.len(),
        source_buf.len()
    );

    // Pipe source -> destination with zero per-record allocation.
    let mut dest_buf = Vec::new();
    let mut reader = DryIceReader::new(source_buf.as_slice())?;
    let mut dest_writer = DryIceWriter::builder()
        .inner(&mut dest_buf)
        .target_block_records(20)
        .build();

    let mut piped = 0;
    while reader.next_record()? {
        dest_writer.write_record(&reader)?;
        piped += 1;
    }
    dest_writer.finish()?;

    println!("Destination: {piped} records, {} bytes", dest_buf.len());
    println!(
        "Size difference: {} bytes (different block sizes)",
        dest_buf.len().cast_signed() - source_buf.len().cast_signed()
    );

    Ok(())
}

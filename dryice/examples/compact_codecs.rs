//! Compact codec usage for space-efficient temporary storage.
//!
//! This example demonstrates using non-default codecs to reduce the
//! on-disk footprint of temporary dryice files. It compares the size
//! of raw storage against 2-bit exact encoding with binned quality
//! and split names.
//!
//! Run with: `cargo run --example compact_codecs`

use dryice::{
    BinnedQualityCodec, DryIceReader, DryIceWriter, SeqRecord, SeqRecordLike, SplitNameCodec,
    TwoBitExactCodec,
};

#[allow(clippy::cast_precision_loss)]
fn main() -> Result<(), dryice::DryIceError> {
    let records: Vec<SeqRecord> = (0..1000)
        .map(|i| {
            let name = format!("instrument:run:flowcell:1:100:{i}:500 1:N:0:ATCACG").into_bytes();
            let seq = b"ACGTACGTACGTACGT".repeat(10);
            let qual = vec![b'I'; seq.len()];
            SeqRecord::new(name, seq, qual).expect("valid record")
        })
        .collect();

    // Write with default raw codecs.
    let mut raw_buf = Vec::new();
    let mut writer = DryIceWriter::builder().inner(&mut raw_buf).build();
    for record in &records {
        writer.write_record(record)?;
    }
    writer.finish()?;

    // Write with compact codecs.
    let mut compact_buf = Vec::new();
    let mut writer = DryIceWriter::builder()
        .inner(&mut compact_buf)
        .two_bit_exact()
        .binned_quality()
        .split_names()
        .build();
    for record in &records {
        writer.write_record(record)?;
    }
    writer.finish()?;

    println!("Records:     {}", records.len());
    println!("Raw size:    {} bytes", raw_buf.len());
    println!("Compact size: {} bytes", compact_buf.len());
    let ratio = 100.0 * compact_buf.len() as f64 / raw_buf.len() as f64;
    println!("Ratio:       {ratio:.1}%");

    // Verify compact round-trip.
    let mut reader =
        DryIceReader::with_codecs::<TwoBitExactCodec, BinnedQualityCodec, SplitNameCodec>(
            compact_buf.as_slice(),
        )?;
    let mut count = 0;
    while reader.next_record()? {
        assert_eq!(reader.sequence().len(), 160);
        count += 1;
    }
    println!("Verified {count} records from compact file");

    Ok(())
}

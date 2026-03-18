//! Newtype adapter pattern for using dryice with noodles-fastq.
//!
//! This example demonstrates the recommended pattern for integrating
//! dryice with the noodles FASTQ library. Rather than depending on
//! an adapter crate, users write a thin newtype wrapper with a Deref
//! impl and a `SeqRecordLike` impl. This keeps the user in control of
//! which noodles version they use and avoids semver coupling.
//!
//! Run with: `cargo run --example noodles_adapter`

use std::ops::Deref;

use dryice::{DryIceReader, DryIceWriter, SeqRecordLike};

/// A newtype wrapper around a noodles FASTQ record that implements
/// `SeqRecordLike` for use with dryice.
struct NoodlesRecord(noodles_fastq::Record);

impl Deref for NoodlesRecord {
    type Target = noodles_fastq::Record;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl SeqRecordLike for NoodlesRecord {
    fn name(&self) -> &[u8] {
        self.0.name()
    }

    fn sequence(&self) -> &[u8] {
        self.0.sequence()
    }

    fn quality(&self) -> &[u8] {
        self.0.quality_scores()
    }
}

fn main() -> Result<(), dryice::DryIceError> {
    // Simulate parsing FASTQ records with noodles.
    let fastq_data = b"@read1\nACGTACGT\n+\n!!!!!!!!\n@read2\nTGCATGCA\n+\n########\n";

    let mut noodles_records = Vec::new();
    let mut reader = noodles_fastq::io::Reader::new(&fastq_data[..]);
    for result in reader.records() {
        let record = result.expect("valid FASTQ record");
        noodles_records.push(record);
    }

    println!("Parsed {} records with noodles", noodles_records.len());

    // Write them into dryice using the newtype wrapper.
    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder().inner(&mut buf).build();

    for record in &noodles_records {
        let wrapped = NoodlesRecord(record.clone());
        writer.write_record(&wrapped)?;
    }
    writer.finish()?;

    println!("Wrote {} bytes to dryice format", buf.len());

    // Read them back.
    let mut reader = DryIceReader::new(buf.as_slice())?;
    let mut count = 0;
    while reader.next_record()? {
        let name = std::str::from_utf8(reader.name()).unwrap_or("<non-utf8>");
        let seq_len = reader.sequence().len();
        println!("  {name}: {seq_len} bp");
        count += 1;
    }

    println!("Read back {count} records");
    Ok(())
}

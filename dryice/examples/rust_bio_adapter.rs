//! Newtype adapter pattern for using dryice with rust-bio.
//!
//! This example demonstrates the recommended pattern for integrating
//! dryice with the rust-bio library's FASTQ reader. The same thin
//! newtype + Deref + `SeqRecordLike` pattern works for any library
//! that provides a FASTQ record type.
//!
//! Run with: `cargo run --example rust_bio_adapter`

use std::ops::Deref;

use bio::io::fastq;
use dryice::{DryIceReader, DryIceWriter, SeqRecordLike};

/// A newtype wrapper around a rust-bio FASTQ record that implements
/// `SeqRecordLike` for use with dryice.
struct BioRecord(fastq::Record);

impl Deref for BioRecord {
    type Target = fastq::Record;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl SeqRecordLike for BioRecord {
    fn name(&self) -> &[u8] {
        self.0.id().as_bytes()
    }

    fn sequence(&self) -> &[u8] {
        self.0.seq()
    }

    fn quality(&self) -> &[u8] {
        self.0.qual()
    }
}

fn main() -> Result<(), dryice::DryIceError> {
    // Simulate parsing FASTQ records with rust-bio.
    let fastq_data = b"@read1\nACGTACGT\n+\n!!!!!!!!\n@read2\nTGCATGCA\n+\n########\n";

    let bio_reader = fastq::Reader::new(&fastq_data[..]);
    let bio_records: Vec<fastq::Record> = bio_reader
        .records()
        .collect::<Result<Vec<_>, _>>()
        .expect("valid FASTQ");

    println!("Parsed {} records with rust-bio", bio_records.len());

    // Write them into dryice using the newtype wrapper.
    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder().inner(&mut buf).build();

    for record in &bio_records {
        let wrapped = BioRecord(record.clone());
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

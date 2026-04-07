//! Key-only minimizer persistence with empty payload.
//!
//! This example shows the most compact storage mode unlocked by the new
//! payload-shaping APIs: keep only one derived key per record and omit the row
//! payload entirely. This is useful for intermediate spill/reload workflows
//! where the sequence has already served its purpose and only the derived key
//! needs to survive on disk.
//!
//! Run with: `cargo run --example key_only_kmers`

use dryice::{DefaultMinimizer64, DryIceReader, DryIceWriter, SeqRecord};

fn main() -> Result<(), dryice::DryIceError> {
    let records = [
        SeqRecord::new(
            b"r1".to_vec(),
            b"ACGTGCTCAGAGACTCAGAGGATTACAGTTTACGTGCTCAGAGACTCAGAGGA".to_vec(),
            vec![b'!'; 53],
        )?,
        SeqRecord::new(
            b"r2".to_vec(),
            b"TGCATGCATGCATGCATGCATGCATGCATGCATGCATGCATGCATGCATGCAT".to_vec(),
            vec![b'#'; 53],
        )?,
    ];

    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder().inner(&mut buf).minimizers().build();

    for record in &records {
        if let Some(key) = DefaultMinimizer64::try_from_sequence(record.sequence())? {
            writer.write_key_only(&key)?;
        }
    }
    writer.finish()?;

    println!(
        "wrote {} bytes for {} key-only records",
        buf.len(),
        records.len()
    );

    let mut reader = DryIceReader::builder()
        .inner(buf.as_slice())
        .sequence_codec::<dryice::OmittedSequenceCodec>()
        .quality_codec::<dryice::OmittedQualityCodec>()
        .name_codec::<dryice::OmittedNameCodec>()
        .record_key::<DefaultMinimizer64>()
        .build()?;

    while let Some(key) = reader.next_key()? {
        println!("minimizer={:#018x}", key.0);
    }

    Ok(())
}

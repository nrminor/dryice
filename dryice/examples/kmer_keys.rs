//! Progressive-disclosure builder APIs for kmer-derived record keys.
//!
//! This example demonstrates the new built-in packed canonical key families
//! for prefixes and minimizers, along with the builder conveniences
//! that choose those key types. It keeps the write path honest: keys are still
//! derived explicitly and written through the normal keyed writer API.
//!
//! Run with: `cargo run --example kmer_keys`

use dryice::{
    DefaultMinimizer64, DryIceReader, DryIceWriter, Minimizer64, SeqRecord, SeqRecordLike,
};

fn main() -> Result<(), dryice::DryIceError> {
    let record = SeqRecord::new(
        b"read1".to_vec(),
        b"ACGTGCTCAGAGACTCAGAGGATTACAGTTTACGTGCTCAGAGACTCAGAGGA".to_vec(),
        vec![b'!'; 53],
    )?;

    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder()
        .inner(&mut buf)
        .minimizers_with_sequences()
        .build();

    // Equivalent builder choices:
    //   .minimizer_key_default()
    //   .omit_quality()
    //   .omit_names()

    if let Some(key) = DefaultMinimizer64::try_from_sequence(record.sequence())? {
        writer.write_record_with_key(&record, &key)?;
    }

    writer.finish()?;

    let mut reader = DryIceReader::builder()
        .inner(buf.as_slice())
        .record_key::<DefaultMinimizer64>()
        .build()?;

    while reader.next_record()? {
        let key = reader.record_key()?;
        println!(
            "name={} minimizer={:#018x}",
            std::str::from_utf8(reader.name()).unwrap_or("<non-utf8>"),
            key.0
        );
    }

    // Power-user equivalent:
    let _writer = DryIceWriter::builder()
        .inner(Vec::new())
        .minimizer_key::<31, 15>()
        .build();

    let _key = Minimizer64::<31, 15>::try_from_sequence(record.sequence())?;

    Ok(())
}

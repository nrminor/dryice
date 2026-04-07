//! Minimizer keys with names retained and payload otherwise omitted.
//!
//! This example demonstrates that the new payload-shaping APIs are not limited
//! to the extreme key-only case. Here we keep only record names alongside
//! minimizer keys, which is often enough for lightweight traceability while
//! still staying much smaller than a full read payload.
//!
//! Run with: `cargo run --example kmer_name_pairs`

use dryice::{DefaultMinimizer64, DryIceReader, DryIceWriter, SeqRecord, SeqRecordLike};

fn main() -> Result<(), dryice::DryIceError> {
    let records = [
        SeqRecord::new(
            b"read_alpha".to_vec(),
            b"ACGTGCTCAGAGACTCAGAGGATTACAGTTTACGTGCTCAGAGACTCAGAGGA".to_vec(),
            vec![b'!'; 53],
        )?,
        SeqRecord::new(
            b"read_beta".to_vec(),
            b"TGCATGCATGCATGCATGCATGCATGCATGCATGCATGCATGCATGCATGCAT".to_vec(),
            vec![b'#'; 53],
        )?,
    ];

    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder()
        .inner(&mut buf)
        .minimizers_with_names()
        .build();

    for record in &records {
        if let Some(key) = DefaultMinimizer64::try_from_sequence(record.sequence())? {
            writer.write_record_with_key(record, &key)?;
        }
    }
    writer.finish()?;

    let mut reader = DryIceReader::builder()
        .inner(buf.as_slice())
        .sequence_codec::<dryice::OmittedSequenceCodec>()
        .quality_codec::<dryice::OmittedQualityCodec>()
        .record_key::<DefaultMinimizer64>()
        .build()?;

    while reader.next_record()? {
        println!(
            "name={} minimizer={:#018x}",
            std::str::from_utf8(reader.name()).unwrap_or("<non-utf8>"),
            reader.record_key()?.0
        );
        assert_eq!(reader.sequence(), b"");
        assert_eq!(reader.quality(), b"");
    }

    Ok(())
}

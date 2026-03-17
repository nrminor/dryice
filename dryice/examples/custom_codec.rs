//! Custom codec implementation.
//!
//! This example demonstrates implementing a custom `SequenceCodec`.
//! The example codec is a simple run-length encoder for sequences
//! with long homopolymer runs — not production-quality, but enough
//! to show the pattern.
//!
//! Run with: `cargo run --example custom_codec`

use dryice::{DryIceError, DryIceReader, DryIceWriter, SeqRecord, SeqRecordLike, SequenceCodec};

struct RunLengthCodec;

impl SequenceCodec for RunLengthCodec {
    const TYPE_TAG: [u8; 16] = *b"demo:seq:rle!!!!";
    const LOSSY: bool = false;

    fn encode(sequence: &[u8]) -> Result<Vec<u8>, DryIceError> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < sequence.len() {
            let base = sequence[i];
            let mut count: u8 = 1;
            while i + usize::from(count) < sequence.len()
                && sequence[i + usize::from(count)] == base
                && count < 255
            {
                count += 1;
            }
            out.push(base);
            out.push(count);
            i += usize::from(count);
        }
        Ok(out)
    }

    fn decode(encoded: &[u8], _original_len: usize) -> Result<Vec<u8>, DryIceError> {
        let mut out = Vec::new();
        for chunk in encoded.chunks_exact(2) {
            let base = chunk[0];
            let count = chunk[1];
            for _ in 0..count {
                out.push(base);
            }
        }
        Ok(out)
    }
}

fn main() -> Result<(), DryIceError> {
    let records = vec![
        SeqRecord::new(
            b"homopolymer".to_vec(),
            b"AAAAAAAAACCCCCCCCCGGGGGGGGG".to_vec(),
            b"!!!!!!!!!!!!!!!!!!!!!!!!!!!".to_vec(),
        )?,
        SeqRecord::new(
            b"mixed".to_vec(),
            b"ACGTACGTACGTACGT".to_vec(),
            b"!!!!!!!!!!!!!!!!".to_vec(),
        )?,
    ];

    // Write with the custom RLE codec.
    let mut rle_buf = Vec::new();
    let mut writer = DryIceWriter::builder()
        .inner(&mut rle_buf)
        .sequence_codec::<RunLengthCodec>()
        .build();
    for record in &records {
        writer.write_record(record)?;
    }
    writer.finish()?;

    // Write with raw for comparison.
    let mut raw_buf = Vec::new();
    let mut writer = DryIceWriter::builder().inner(&mut raw_buf).build();
    for record in &records {
        writer.write_record(record)?;
    }
    writer.finish()?;

    println!("Raw size: {} bytes", raw_buf.len());
    println!("RLE size: {} bytes", rle_buf.len());

    // Read back with the custom codec.
    let mut reader =
        DryIceReader::with_codecs::<RunLengthCodec, dryice::RawQualityCodec, dryice::RawNameCodec>(
            rle_buf.as_slice(),
        )?;

    while reader.next_record()? {
        let name = std::str::from_utf8(reader.name()).unwrap_or("<non-utf8>");
        println!("  {name}: {} bp", reader.sequence().len());
    }

    Ok(())
}

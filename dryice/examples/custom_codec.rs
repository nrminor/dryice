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

    fn encode_into(sequence: &[u8], output: &mut Vec<u8>) -> Result<(), DryIceError> {
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
            output.push(base);
            output.push(count);
            i += usize::from(count);
        }
        Ok(())
    }

    fn decode_into(
        encoded: &[u8],
        _original_len: usize,
        output: &mut Vec<u8>,
    ) -> Result<(), DryIceError> {
        for chunk in encoded.chunks_exact(2) {
            let base = chunk[0];
            let count = chunk[1];
            for _ in 0..count {
                output.push(base);
            }
        }
        Ok(())
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

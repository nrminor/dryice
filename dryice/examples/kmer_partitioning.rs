//! Partition records by a real kmer-derived key.
//!
//! This example upgrades the older first-base partitioning story into a more
//! domain-aware flow by deriving a packed canonical prefix kmer key and using it
//! to choose a partition bucket. The resulting files retain names only, which is
//! often a useful compromise for later inspection while still keeping the on-disk
//! representation compact.
//!
//! Run with: `cargo run --example kmer_partitioning`

use dryice::{DefaultPrefixKmer64, DryIceReader, DryIceWriter, SeqRecord};

fn main() -> Result<(), dryice::DryIceError> {
    let records = [
        SeqRecord::new(
            b"r1".to_vec(),
            b"ACGTGCTCAGAGACTCAGAGGATTACAGTTT".to_vec(),
            vec![b'!'; 31],
        )?,
        SeqRecord::new(
            b"r2".to_vec(),
            b"TGCATGCATGCATGCATGCATGCATGCATGC".to_vec(),
            vec![b'#'; 31],
        )?,
        SeqRecord::new(
            b"r3".to_vec(),
            b"CCCCAAAATTTTGGGGCCCCAAAATTTTGGG".to_vec(),
            vec![b'$'; 31],
        )?,
        SeqRecord::new(
            b"r4".to_vec(),
            b"GGGGTTTTAAAACCCCGGGGTTTTAAAACCC".to_vec(),
            vec![b'%'; 31],
        )?,
    ];

    let mut buckets: Vec<Vec<u8>> = vec![Vec::new(); 4];
    let mut writers: Vec<_> = buckets
        .iter_mut()
        .map(|buf| {
            DryIceWriter::builder()
                .inner(buf)
                .prefix_kmers_with_names()
                .build()
        })
        .collect();

    for record in &records {
        if let Some(key) = DefaultPrefixKmer64::try_from_sequence(record.sequence())? {
            let bucket = (key.0 as usize) % writers.len();
            writers[bucket].write_record_with_key(record, &key)?;
        }
    }

    for writer in writers {
        writer.finish()?;
    }

    for (i, buf) in buckets.iter().enumerate() {
        let mut reader = DryIceReader::builder()
            .inner(buf.as_slice())
            .sequence_codec::<dryice::OmittedSequenceCodec>()
            .quality_codec::<dryice::OmittedQualityCodec>()
            .record_key::<DefaultPrefixKmer64>()
            .build()?;

        let mut count = 0usize;
        while reader.next_record()? {
            count += 1;
        }
        println!("bucket {i}: {count} records ({} bytes)", buf.len());
    }

    Ok(())
}

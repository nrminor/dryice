//! Partition records by a real kmer-derived key.
//!
//! This example upgrades the older first-base partitioning story into a more
//! domain-aware flow by deriving a packed canonical prefix kmer key and using it
//! to choose a partition bucket. The resulting owned temporary files retain names
//! only, which is often a useful compromise for later inspection while still
//! keeping the on-disk representation compact.
//!
//! Run with: `cargo run --example kmer_partitioning`

use dryice::{DefaultPrefixKmer64, DryIceReader, DryIceWriter, SeqRecord, TempDryIceFile};

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

    let buckets: Vec<TempDryIceFile> = (0..4)
        .map(|_| TempDryIceFile::new())
        .collect::<Result<Vec<_>, _>>()?;
    let mut writers: Vec<_> = buckets
        .iter()
        .map(|bucket| {
            let file = bucket.open()?;
            Ok::<_, dryice::DryIceError>(
                DryIceWriter::builder()
                    .inner(file)
                    .prefix_kmers_with_names()
                    .build(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    for record in &records {
        if let Some(key) = DefaultPrefixKmer64::try_from_sequence(record.sequence())? {
            let bucket = usize::try_from(key.0).expect("prefix kmer key should fit in usize")
                % writers.len();
            writers[bucket].write_record_with_key(record, &key)?;
        }
    }

    for writer in writers {
        writer.finish()?;
    }

    for (i, bucket) in buckets.iter().enumerate() {
        let file = bucket.open()?;
        let mut reader = DryIceReader::builder()
            .inner(file)
            .sequence_codec::<dryice::OmittedSequenceCodec>()
            .quality_codec::<dryice::OmittedQualityCodec>()
            .record_key::<DefaultPrefixKmer64>()
            .build()?;

        let mut count = 0usize;
        while reader.next_record()? {
            count += 1;
        }
        let bytes = bucket.path().metadata()?.len();
        println!("bucket {i}: {count} records ({bytes} bytes)");
    }

    for bucket in buckets {
        bucket.cleanup()?;
    }

    Ok(())
}

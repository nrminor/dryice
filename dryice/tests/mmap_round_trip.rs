//! Memory-mapped reader round-trip tests.

#![cfg(feature = "mmap")]

use std::io::Write;

use dryice::{DryIceWriter, MmapDryIceReader, SeqRecord};

#[test]
fn mmap_read_default_codecs() {
    let records = vec![
        SeqRecord::new(b"r1".to_vec(), b"ACGT".to_vec(), b"!!!!".to_vec()).expect("valid record"),
        SeqRecord::new(b"r2".to_vec(), b"TGCA".to_vec(), b"####".to_vec()).expect("valid record"),
    ];

    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder().inner(&mut buf).build();
    for record in &records {
        writer.write_record(record).expect("write should succeed");
    }
    writer.finish().expect("finish should succeed");

    let mut tmpfile = tempfile::NamedTempFile::new().expect("temp file");
    tmpfile.write_all(&buf).expect("write temp file");
    tmpfile.flush().expect("flush temp file");

    let file = std::fs::File::open(tmpfile.path()).expect("open temp file");
    let mut reader = MmapDryIceReader::open(&file).expect("mmap reader should open");

    let mut read_back = Vec::new();
    while reader.next_record().expect("next_record should succeed") {
        read_back.push(
            dryice::SeqRecordExt::to_seq_record(&reader).expect("to_seq_record should succeed"),
        );
    }

    assert_eq!(read_back.len(), records.len());
    for (orig, back) in records.iter().zip(read_back.iter()) {
        assert_eq!(orig.name(), back.name());
        assert_eq!(orig.sequence(), back.sequence());
        assert_eq!(orig.quality(), back.quality());
    }
}

#[test]
fn mmap_read_multiple_blocks() {
    let records: Vec<SeqRecord> = (0..20)
        .map(|i| {
            SeqRecord::new(
                format!("read_{i}").into_bytes(),
                b"ACGTACGT".to_vec(),
                b"!!!!!!!!".to_vec(),
            )
            .expect("valid record")
        })
        .collect();

    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder()
        .inner(&mut buf)
        .target_block_records(5)
        .build();
    for record in &records {
        writer.write_record(record).expect("write should succeed");
    }
    writer.finish().expect("finish should succeed");

    let mut tmpfile = tempfile::NamedTempFile::new().expect("temp file");
    tmpfile.write_all(&buf).expect("write temp file");
    tmpfile.flush().expect("flush temp file");

    let file = std::fs::File::open(tmpfile.path()).expect("open temp file");
    let reader = MmapDryIceReader::open(&file).expect("mmap reader should open");
    let read_back = reader.into_records().expect("into_records should succeed");

    assert_eq!(read_back.len(), 20);
    for (i, record) in read_back.iter().enumerate() {
        assert_eq!(record.name(), format!("read_{i}").as_bytes());
    }
}

#[test]
fn mmap_read_empty_file() {
    let mut buf = Vec::new();
    let writer = DryIceWriter::builder().inner(&mut buf).build();
    writer.finish().expect("finish should succeed");

    let mut tmpfile = tempfile::NamedTempFile::new().expect("temp file");
    tmpfile.write_all(&buf).expect("write temp file");
    tmpfile.flush().expect("flush temp file");

    let file = std::fs::File::open(tmpfile.path()).expect("open temp file");
    let reader = MmapDryIceReader::open(&file).expect("mmap reader should open");
    let records = reader.into_records().expect("into_records should succeed");
    assert!(records.is_empty());
}

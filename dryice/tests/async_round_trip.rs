//! Async round-trip tests for the dryice format.

#![cfg(feature = "async")]

use dryice::{AsyncDryIceReader, DryIceWriter, SeqRecord, SeqRecordLike};

#[tokio::test]
async fn async_write_and_read_default_codecs() {
    let records = vec![
        SeqRecord::new(b"r1".to_vec(), b"ACGT".to_vec(), b"!!!!".to_vec()).expect("valid record"),
        SeqRecord::new(b"r2".to_vec(), b"TGCA".to_vec(), b"####".to_vec()).expect("valid record"),
    ];

    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder().inner(&mut buf).build_async();

    for record in &records {
        writer
            .write_record(record)
            .await
            .expect("write should succeed");
    }
    writer.finish().await.expect("finish should succeed");

    assert!(buf.len() >= 8, "should contain at least the file header");

    let mut reader = AsyncDryIceReader::new(buf.as_slice())
        .await
        .expect("reader should open");

    let mut read_back = Vec::new();
    while reader
        .next_record()
        .await
        .expect("next_record should succeed")
    {
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

#[tokio::test]
async fn async_write_sync_read_interop() {
    let records = vec![
        SeqRecord::new(b"r1".to_vec(), b"ACGTACGT".to_vec(), b"!!!!!!!!".to_vec())
            .expect("valid record"),
    ];

    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder().inner(&mut buf).build_async();

    for record in &records {
        writer
            .write_record(record)
            .await
            .expect("write should succeed");
    }
    writer.finish().await.expect("finish should succeed");

    let mut sync_reader =
        dryice::DryIceReader::new(buf.as_slice()).expect("sync reader should open");
    assert!(
        sync_reader
            .next_record()
            .expect("next_record should succeed")
    );
    assert_eq!(sync_reader.sequence(), b"ACGTACGT");
}

#[tokio::test]
async fn sync_write_async_read_interop() {
    let records = vec![
        SeqRecord::new(b"r1".to_vec(), b"ACGTACGT".to_vec(), b"!!!!!!!!".to_vec())
            .expect("valid record"),
    ];

    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder().inner(&mut buf).build();
    for record in &records {
        writer.write_record(record).expect("write should succeed");
    }
    writer.finish().expect("finish should succeed");

    let mut async_reader = AsyncDryIceReader::new(buf.as_slice())
        .await
        .expect("async reader should open");
    assert!(
        async_reader
            .next_record()
            .await
            .expect("next_record should succeed")
    );
    assert_eq!(async_reader.sequence(), b"ACGTACGT");
}

#[tokio::test]
async fn async_empty_file() {
    let mut buf = Vec::new();
    let writer = DryIceWriter::builder().inner(&mut buf).build_async();
    writer.finish().await.expect("finish should succeed");

    let reader = AsyncDryIceReader::new(buf.as_slice())
        .await
        .expect("reader should open");
    let records = reader
        .into_records()
        .await
        .expect("into_records should succeed");
    assert!(records.is_empty());
}

#[tokio::test]
async fn async_multiple_blocks() {
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
        .build_async();

    for record in &records {
        writer
            .write_record(record)
            .await
            .expect("write should succeed");
    }
    writer.finish().await.expect("finish should succeed");

    let reader = AsyncDryIceReader::new(buf.as_slice())
        .await
        .expect("reader should open");
    let read_back = reader
        .into_records()
        .await
        .expect("into_records should succeed");

    assert_eq!(read_back.len(), 20);
    for (i, record) in read_back.iter().enumerate() {
        assert_eq!(record.name(), format!("read_{i}").as_bytes());
    }
}

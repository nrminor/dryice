//! Temporary-file lifecycle tests.

use std::fs;

use dryice::{DryIceReader, DryIceWriter, SeqRecord, SeqRecordExt, TempDryIceFile};

#[test]
fn temp_file_round_trips_records_and_cleans_up_explicitly() {
    let temp = TempDryIceFile::new().expect("temp file should be created");
    let path = temp.path().to_path_buf();

    let records = vec![
        SeqRecord::new(b"r1".to_vec(), b"ACGT".to_vec(), b"!!!!".to_vec()).expect("valid record"),
        SeqRecord::new(b"r2".to_vec(), b"TGCA".to_vec(), b"####".to_vec()).expect("valid record"),
    ];

    {
        let file = temp.open().expect("temp file should open");
        let mut writer = DryIceWriter::builder().inner(file).build();
        for record in &records {
            writer
                .write_record(record)
                .expect("write_record should succeed");
        }
        writer.finish().expect("finish should succeed");
    }

    let read_back = {
        let file = temp.open().expect("temp file should open");
        let mut reader = DryIceReader::new(file).expect("reader should open");
        let mut read_back = Vec::new();
        while reader.next_record().expect("next_record should succeed") {
            read_back.push(
                reader
                    .to_seq_record()
                    .expect("to_seq_record should succeed"),
            );
        }
        read_back
    };

    assert_eq!(read_back.len(), records.len());
    for (expected, actual) in records.iter().zip(read_back.iter()) {
        assert_eq!(expected.name(), actual.name());
        assert_eq!(expected.sequence(), actual.sequence());
        assert_eq!(expected.quality(), actual.quality());
    }

    assert!(path.exists(), "temporary file should exist before cleanup");
    temp.cleanup().expect("cleanup should succeed");
    assert!(!path.exists(), "temporary file should be removed");
}

#[test]
fn temp_file_cleans_up_on_drop() {
    let path = {
        let temp = TempDryIceFile::new().expect("temp file should be created");
        let path = temp.path().to_path_buf();
        assert!(path.exists(), "temporary file should exist while owned");
        path
    };

    assert!(!path.exists(), "temporary file should be removed on drop");
}

#[test]
fn explicit_cleanup_treats_missing_file_as_success() {
    let temp = TempDryIceFile::new().expect("temp file should be created");
    let path = temp.path().to_path_buf();

    fs::remove_file(&path).expect("manual removal should succeed");
    temp.cleanup()
        .expect("cleanup should accept an already-missing temp file");
}

#[test]
fn persist_moves_file_and_disarms_cleanup() {
    let directory = tempfile::tempdir().expect("tempdir should be created");
    let destination = directory.path().join("kept.dryice");

    let persisted_path = {
        let mut temp =
            TempDryIceFile::new_in(directory.path()).expect("temp file should be created");
        let original_path = temp.path().to_path_buf();

        {
            let file = temp.open().expect("temp file should open");
            let writer = DryIceWriter::builder().inner(file).build();
            writer.finish().expect("finish should succeed");
        }

        let persisted_path = temp
            .persist(&destination)
            .expect("persist should move the file");
        assert_eq!(persisted_path, destination);
        assert!(!original_path.exists(), "persist should move the temp path");
        persisted_path
    };

    assert!(
        persisted_path.exists(),
        "persisted file should remain caller-owned"
    );
}

#[test]
fn persist_rejects_existing_destination() {
    let directory = tempfile::tempdir().expect("tempdir should be created");
    let destination = directory.path().join("existing.dryice");
    fs::write(&destination, b"already here").expect("destination fixture should be written");

    let mut temp = TempDryIceFile::new_in(directory.path()).expect("temp file should be created");
    let original_path = temp.path().to_path_buf();

    let error = temp
        .persist(&destination)
        .expect_err("persist should reject existing destinations");

    assert!(
        error
            .to_string()
            .contains("persist destination already exists"),
        "unexpected error: {error}"
    );
    assert!(
        original_path.exists(),
        "failed persist should leave the temp file for drop cleanup"
    );
}

//! Integration tests for dryice format round-trip fidelity.

use dryice::{
    Bytes8Key, Bytes16Key, DryIceReader, DryIceWriter, DryIceWriterOptions, RecordKey, SeqRecord,
    SeqRecordExt, SeqRecordLike,
};
use proptest::prelude::*;

/// Assert that two record slices are identical field-by-field.
fn assert_records_equal(expected: &[SeqRecord], actual: &[SeqRecord]) {
    assert_eq!(
        expected.len(),
        actual.len(),
        "record count mismatch: expected {}, got {}",
        expected.len(),
        actual.len()
    );
    for (i, (exp, act)) in expected.iter().zip(actual.iter()).enumerate() {
        assert_eq!(exp.name(), act.name(), "name mismatch at record {i}");
        assert_eq!(
            exp.sequence(),
            act.sequence(),
            "sequence mismatch at record {i}"
        );
        assert_eq!(
            exp.quality(),
            act.quality(),
            "quality mismatch at record {i}"
        );
    }
}

/// Write records and read them back using the zero-copy primary path.
fn round_trip_zero_copy(records: &[SeqRecord], block_size: usize) -> Vec<SeqRecord> {
    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder()
        .inner(&mut buf)
        .target_block_records(block_size)
        .build();

    for record in records {
        writer
            .write_record(record)
            .expect("write_record should succeed");
    }
    writer.finish().expect("finish should succeed");

    assert!(
        buf.len() >= 8,
        "output buffer should contain at least the file header"
    );

    let mut reader = DryIceReader::new(buf.as_slice()).expect("reader should open");
    let mut result = Vec::new();
    while reader.next_record().expect("next_record should succeed") {
        result.push(
            reader
                .to_seq_record()
                .expect("to_seq_record should succeed"),
        );
    }
    result
}

/// Write records and read them back using the convenience iterator.
fn round_trip_iterator(records: &[SeqRecord], block_size: usize) -> Vec<SeqRecord> {
    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder()
        .inner(&mut buf)
        .target_block_records(block_size)
        .build();

    for record in records {
        writer
            .write_record(record)
            .expect("write_record should succeed");
    }
    writer.finish().expect("finish should succeed");

    let reader = DryIceReader::new(buf.as_slice()).expect("reader should open");
    reader
        .into_records()
        .collect::<Result<Vec<_>, _>>()
        .expect("all records should decode")
}

fn round_trip_zero_copy_keyed<K>(
    records: &[SeqRecord],
    keys: &[K],
    block_size: usize,
) -> (Vec<SeqRecord>, Vec<K>)
where
    K: RecordKey + Clone,
{
    assert_eq!(records.len(), keys.len(), "records and keys must align");

    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder()
        .inner(&mut buf)
        .record_key::<K>()
        .target_block_records(block_size)
        .build();

    for (record, key) in records.iter().zip(keys.iter()) {
        writer
            .write_record_with_key(record, key)
            .expect("write_record_with_key should succeed");
    }
    writer.finish().expect("finish should succeed");

    let mut reader =
        DryIceReader::with_record_key::<K>(buf.as_slice()).expect("reader should open");
    let mut out_records = Vec::new();
    let mut out_keys = Vec::new();

    while reader.next_record().expect("next_record should succeed") {
        out_records.push(
            reader
                .to_seq_record()
                .expect("to_seq_record should succeed"),
        );
        out_keys.push(reader.record_key().expect("record_key should decode"));
    }

    (out_records, out_keys)
}

#[test]
fn zero_copy_round_trip_single_record() {
    let records = vec![
        SeqRecord::new(b"r1".to_vec(), b"ACGT".to_vec(), b"!!!!".to_vec()).expect("valid record"),
    ];
    let read_back = round_trip_zero_copy(&records, 100);
    assert_records_equal(&records, &read_back);
}

#[test]
fn zero_copy_round_trip_single_block() {
    let records = vec![
        SeqRecord::new(b"read1".to_vec(), b"ACGT".to_vec(), b"!!!!".to_vec())
            .expect("valid record"),
        SeqRecord::new(b"read2".to_vec(), b"TGCA".to_vec(), b"####".to_vec())
            .expect("valid record"),
        SeqRecord::new(
            b"read3".to_vec(),
            b"AAACCCGGGTTT".to_vec(),
            b"!!!###$$$%%%".to_vec(),
        )
        .expect("valid record"),
    ];
    let read_back = round_trip_zero_copy(&records, 100);
    assert_records_equal(&records, &read_back);
}

#[test]
fn zero_copy_round_trip_multiple_blocks() {
    let records: Vec<SeqRecord> = (0..10)
        .map(|i| {
            let name = format!("read_{i}").into_bytes();
            let seq = b"ACGTACGT".to_vec();
            let qual = b"!!!!####".to_vec();
            SeqRecord::new(name, seq, qual).expect("valid record")
        })
        .collect();

    let read_back = round_trip_zero_copy(&records, 3);
    assert_records_equal(&records, &read_back);
}

#[test]
fn zero_copy_round_trip_empty_file() {
    let read_back = round_trip_zero_copy(&[], 100);
    assert!(read_back.is_empty());
}

#[test]
fn zero_copy_round_trip_empty_name() {
    let records =
        vec![SeqRecord::new(Vec::new(), b"ACGT".to_vec(), b"!!!!".to_vec()).expect("valid record")];
    let read_back = round_trip_zero_copy(&records, 100);
    assert_records_equal(&records, &read_back);
    assert!(read_back[0].name().is_empty());
}

#[test]
fn zero_copy_round_trip_long_sequence() {
    let seq = b"ACGT".repeat(2500); // 10,000 bases
    let qual = vec![b'!'; seq.len()];
    let records = vec![SeqRecord::new(b"long_read".to_vec(), seq, qual).expect("valid record")];
    let read_back = round_trip_zero_copy(&records, 100);
    assert_records_equal(&records, &read_back);
}

#[test]
fn zero_copy_round_trip_block_boundary_exact() {
    let block_size = 4;
    let records: Vec<SeqRecord> = (0..block_size)
        .map(|i| {
            SeqRecord::new(
                format!("r{i}").into_bytes(),
                b"ACGT".to_vec(),
                b"!!!!".to_vec(),
            )
            .expect("valid record")
        })
        .collect();

    let read_back = round_trip_zero_copy(&records, block_size);
    assert_records_equal(&records, &read_back);
}

#[test]
fn iterator_round_trip_single_block() {
    let records = vec![
        SeqRecord::new(b"read1".to_vec(), b"ACGT".to_vec(), b"!!!!".to_vec())
            .expect("valid record"),
        SeqRecord::new(b"read2".to_vec(), b"TGCA".to_vec(), b"####".to_vec())
            .expect("valid record"),
    ];
    let read_back = round_trip_iterator(&records, 100);
    assert_records_equal(&records, &read_back);
}

#[test]
fn iterator_round_trip_multiple_blocks() {
    let records: Vec<SeqRecord> = (0..10)
        .map(|i| {
            let name = format!("read_{i}").into_bytes();
            let seq = b"ACGTACGT".to_vec();
            let qual = b"!!!!####".to_vec();
            SeqRecord::new(name, seq, qual).expect("valid record")
        })
        .collect();

    let read_back = round_trip_iterator(&records, 3);
    assert_records_equal(&records, &read_back);
}

#[test]
fn iterator_round_trip_empty_file() {
    let read_back = round_trip_iterator(&[], 100);
    assert!(read_back.is_empty());
}

#[test]
fn zero_copy_reader_to_writer_pipe() {
    let records = vec![
        SeqRecord::new(b"r1".to_vec(), b"ACGT".to_vec(), b"!!!!".to_vec()).expect("valid record"),
        SeqRecord::new(b"r2".to_vec(), b"TGCA".to_vec(), b"####".to_vec()).expect("valid record"),
    ];

    // Write original records.
    let mut buf1 = Vec::new();
    let mut writer1 = DryIceWriter::builder().inner(&mut buf1).build();
    for record in &records {
        writer1.write_record(record).expect("write should succeed");
    }
    writer1.finish().expect("finish should succeed");

    // Pipe through reader -> writer with zero-copy.
    let mut buf2 = Vec::new();
    let mut reader = DryIceReader::new(buf1.as_slice()).expect("reader should open");
    let mut writer2 = DryIceWriter::builder().inner(&mut buf2).build();
    while reader.next_record().expect("next_record should succeed") {
        writer2
            .write_record(&reader)
            .expect("pipe write should succeed");
    }
    writer2.finish().expect("finish should succeed");

    // Read back from the second file and verify.
    let read_back = round_trip_zero_copy(&records, 100);
    assert_records_equal(&records, &read_back);
}

#[test]
fn keyed_round_trip_with_built_in_bytes8_key() {
    let records = vec![
        SeqRecord::new(b"r1".to_vec(), b"ACGT".to_vec(), b"!!!!".to_vec()).expect("valid record"),
        SeqRecord::new(b"r2".to_vec(), b"TGCA".to_vec(), b"####".to_vec()).expect("valid record"),
    ];
    let keys = vec![Bytes8Key(*b"key00001"), Bytes8Key(*b"key00002")];

    let (read_back, read_keys) = round_trip_zero_copy_keyed(&records, &keys, 100);
    assert_records_equal(&records, &read_back);
    assert_eq!(keys, read_keys);
}

#[test]
fn keyed_round_trip_with_built_in_bytes16_key_helpers() {
    let records = [
        SeqRecord::new(b"r1".to_vec(), b"ACGT".to_vec(), b"!!!!".to_vec()).expect("valid record"),
    ];
    let keys = [Bytes16Key(*b"bytes16-key-0001")];

    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder()
        .inner(&mut buf)
        .bytes16_key()
        .build();

    writer
        .write_record_with_key(&records[0], &keys[0])
        .expect("write_record_with_key should succeed");
    writer.finish().expect("finish should succeed");

    let mut reader = DryIceReader::with_bytes16_key(buf.as_slice()).expect("reader should open");
    assert!(reader.next_record().expect("next_record should succeed"));
    let key = reader.record_key().expect("record_key should decode");
    assert_eq!(key, keys[0]);
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CustomKey([u8; 12]);

impl RecordKey for CustomKey {
    const WIDTH: u16 = 12;
    const TYPE_TAG: [u8; 16] = *b"dryi:custom:key!";

    fn encode_into(&self, out: &mut [u8]) {
        debug_assert_eq!(out.len(), usize::from(Self::WIDTH));
        out.copy_from_slice(&self.0);
    }

    fn decode_from(bytes: &[u8]) -> Result<Self, dryice::DryIceError> {
        let arr: [u8; 12] =
            bytes
                .try_into()
                .map_err(|_| dryice::DryIceError::InvalidRecordKeyEncoding {
                    message: "invalid custom key length",
                })?;
        Ok(Self(arr))
    }
}

#[test]
fn keyed_round_trip_with_custom_key() {
    let records = vec![
        SeqRecord::new(b"r1".to_vec(), b"ACGT".to_vec(), b"!!!!".to_vec()).expect("valid record"),
        SeqRecord::new(b"r2".to_vec(), b"TGCA".to_vec(), b"####".to_vec()).expect("valid record"),
    ];
    let keys = vec![CustomKey(*b"custom-key-1"), CustomKey(*b"custom-key-2")];

    let (read_back, read_keys) = round_trip_zero_copy_keyed(&records, &keys, 100);
    assert_records_equal(&records, &read_back);
    assert_eq!(keys, read_keys);
}

#[test]
fn seq_record_rejects_mismatched_lengths() {
    let result = SeqRecord::new(
        b"bad".to_vec(),
        b"ACGT".to_vec(),
        b"!!!".to_vec(), // 3 != 4
    );
    assert!(
        result.is_err(),
        "SeqRecord::new should reject mismatched sequence/quality lengths"
    );
}

#[test]
fn writer_rejects_mismatched_record() {
    struct BadRecord;
    impl SeqRecordLike for BadRecord {
        fn name(&self) -> &[u8] {
            b"bad"
        }
        fn sequence(&self) -> &[u8] {
            b"ACGT"
        }
        fn quality(&self) -> &[u8] {
            b"!!!"
        } // 3 != 4
    }

    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder().inner(&mut buf).build();
    let result = writer.write_record(&BadRecord);
    assert!(
        result.is_err(),
        "write_record should reject records with mismatched sequence/quality lengths"
    );
}

#[test]
fn reader_rejects_bad_magic() {
    let bad_data = b"NOPE\x01\x00\x00\x00";
    let result = DryIceReader::new(bad_data.as_slice());
    assert!(
        result.is_err(),
        "DryIceReader::new should reject files with invalid magic bytes"
    );
}

#[test]
fn reader_rejects_truncated_header() {
    let truncated = b"DRY"; // only 3 bytes, need 8
    let result = DryIceReader::new(truncated.as_slice());
    assert!(
        result.is_err(),
        "DryIceReader::new should reject truncated file headers"
    );
}

#[test]
fn from_options_rejects_target_bytes() {
    use dryice::{BlockLayoutOptions, BlockSizePolicy, EncodingOptions};

    let options = DryIceWriterOptions {
        encoding: EncodingOptions::default(),
        layout: BlockLayoutOptions {
            block_size: BlockSizePolicy::TargetBytes(4096),
        },
    };

    let buf = Vec::new();
    let result = DryIceWriter::<_, dryice::RawAsciiCodec, _>::from_options(buf, &options);
    assert!(
        result.is_err(),
        "from_options should reject TargetBytes block size policy"
    );
}

/// Generate a valid `SeqRecord` with arbitrary byte content.
fn arb_seq_record() -> impl Strategy<Value = SeqRecord> {
    (
        prop::collection::vec(any::<u8>(), 0..256),  // name
        prop::collection::vec(any::<u8>(), 1..1024), // sequence (non-empty)
    )
        .prop_map(|(name, seq)| {
            let qual = vec![b'!'; seq.len()];
            SeqRecord::new(name, seq, qual).expect("generated record should be valid")
        })
}

proptest! {
    #[test]
    fn prop_zero_copy_round_trip(
        records in prop::collection::vec(arb_seq_record(), 0..50),
        block_size in 1_usize..20,
    ) {
        let read_back = round_trip_zero_copy(&records, block_size);
        assert_records_equal(&records, &read_back);
    }

    #[test]
    fn prop_iterator_round_trip(
        records in prop::collection::vec(arb_seq_record(), 0..50),
        block_size in 1_usize..20,
    ) {
        let read_back = round_trip_iterator(&records, block_size);
        assert_records_equal(&records, &read_back);
    }
}

fn round_trip_two_bit_exact(records: &[SeqRecord], block_size: usize) -> Vec<SeqRecord> {
    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder()
        .inner(&mut buf)
        .two_bit_exact()
        .target_block_records(block_size)
        .build();

    for record in records {
        writer
            .write_record(record)
            .expect("write_record should succeed");
    }
    writer.finish().expect("finish should succeed");

    let mut reader = DryIceReader::new(buf.as_slice()).expect("reader should open");
    let mut result = Vec::new();
    while reader.next_record().expect("next_record should succeed") {
        result.push(
            reader
                .to_seq_record()
                .expect("to_seq_record should succeed"),
        );
    }
    result
}

#[test]
fn two_bit_exact_round_trip_canonical_only() {
    let records = vec![
        SeqRecord::new(b"r1".to_vec(), b"ACGTACGT".to_vec(), b"!!!!!!!!".to_vec())
            .expect("valid record"),
        SeqRecord::new(b"r2".to_vec(), b"TGCATGCA".to_vec(), b"########".to_vec())
            .expect("valid record"),
    ];
    let read_back = round_trip_two_bit_exact(&records, 100);
    assert_records_equal(&records, &read_back);
}

#[test]
fn two_bit_exact_round_trip_with_ambiguity() {
    let records = vec![
        SeqRecord::new(b"r1".to_vec(), b"ACNGTACGT".to_vec(), b"!!!!!!!!!".to_vec())
            .expect("valid record"),
        SeqRecord::new(b"r2".to_vec(), b"NNNNNN".to_vec(), b"!!!!!!".to_vec())
            .expect("valid record"),
        SeqRecord::new(
            b"r3".to_vec(),
            b"ACGTRYWSMK".to_vec(),
            b"!!!!!!!!!!".to_vec(),
        )
        .expect("valid record"),
    ];
    let read_back = round_trip_two_bit_exact(&records, 100);
    assert_records_equal(&records, &read_back);
}

#[test]
fn two_bit_exact_round_trip_multiple_blocks() {
    let records: Vec<SeqRecord> = (0..10)
        .map(|i| {
            let name = format!("read_{i}").into_bytes();
            let seq = b"ACGTNNACGT".to_vec();
            let qual = b"!!!!!!!!!!".to_vec();
            SeqRecord::new(name, seq, qual).expect("valid record")
        })
        .collect();

    let read_back = round_trip_two_bit_exact(&records, 3);
    assert_records_equal(&records, &read_back);
}

#[test]
fn two_bit_exact_round_trip_long_sequence_with_sparse_ambiguity() {
    let mut seq = b"ACGT".repeat(2500);
    seq[100] = b'N';
    seq[5000] = b'R';
    seq[9999] = b'Y';
    let qual = vec![b'!'; seq.len()];
    let records = vec![SeqRecord::new(b"long".to_vec(), seq, qual).expect("valid record")];
    let read_back = round_trip_two_bit_exact(&records, 100);
    assert_records_equal(&records, &read_back);
}

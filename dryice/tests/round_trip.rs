//! Integration tests for dryice format round-trip fidelity.

use dryice::{
    BinnedQualityCodec, BlockLayoutOptions, BlockSizePolicy, Bytes8Key, Bytes16Key, DryIceReader,
    DryIceWriter, DryIceWriterOptions, QualityCodec, RawAsciiCodec, RawNameCodec, RawQualityCodec,
    RecordKey, SeqRecord, SeqRecordExt, SeqRecordLike, SplitNameCodec,
    fields::{Key, Name, Quality, Sequence},
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
fn selected_reader_sequence_only_scan() {
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

    let mut reader = DryIceReader::builder()
        .inner(buf.as_slice())
        .select(Sequence)
        .build()
        .expect("selected reader should build");

    let mut read_back = Vec::new();
    while let Some(record) = reader.next_record().expect("next_record should succeed") {
        read_back.push(record.sequence().to_vec());
    }

    let expected: Vec<Vec<u8>> = records.iter().map(|r| r.sequence().to_vec()).collect();
    assert_eq!(read_back, expected);
}

#[test]
fn selected_reader_sequence_and_key_scan() {
    let records = vec![
        SeqRecord::new(b"r1".to_vec(), b"ACGT".to_vec(), b"!!!!".to_vec()).expect("valid record"),
        SeqRecord::new(b"r2".to_vec(), b"TGCA".to_vec(), b"####".to_vec()).expect("valid record"),
    ];
    let keys = vec![Bytes8Key(*b"key00001"), Bytes8Key(*b"key00002")];

    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder().inner(&mut buf).bytes8_key().build();
    for (record, key) in records.iter().zip(keys.iter()) {
        writer
            .write_record_with_key(record, key)
            .expect("write should succeed");
    }
    writer.finish().expect("finish should succeed");

    let mut reader = DryIceReader::builder()
        .inner(buf.as_slice())
        .bytes8_key()
        .select(Sequence | Key)
        .build()
        .expect("selected keyed reader should build");

    let mut read_sequences = Vec::new();
    let mut read_keys = Vec::new();
    while let Some(record) = reader.next_record().expect("next_record should succeed") {
        read_sequences.push(record.sequence().to_vec());
        read_keys.push(record.record_key().expect("key should decode"));
    }

    let expected_sequences: Vec<Vec<u8>> = records.iter().map(|r| r.sequence().to_vec()).collect();
    assert_eq!(read_sequences, expected_sequences);
    assert_eq!(read_keys, keys);
}

#[test]
fn selected_reader_all_fields_scan() {
    let record =
        SeqRecord::new(b"r1".to_vec(), b"ACGT".to_vec(), b"!!!!".to_vec()).expect("valid record");

    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder().inner(&mut buf).build();
    writer.write_record(&record).expect("write should succeed");
    writer.finish().expect("finish should succeed");

    let mut reader = DryIceReader::builder()
        .inner(buf.as_slice())
        .select(Name | Sequence | Quality)
        .build()
        .expect("selected reader should build");

    let selected = reader
        .next_record()
        .expect("next_record should succeed")
        .expect("record should exist");
    assert_eq!(selected.name(), record.name());
    assert_eq!(selected.sequence(), record.sequence());
    assert_eq!(selected.quality(), record.quality());
}

#[test]
fn selected_reader_compact_sequence_only_scan() {
    let records = vec![
        SeqRecord::new(b"r1 desc".to_vec(), b"ACGTNN".to_vec(), b"!!III#".to_vec())
            .expect("valid record"),
        SeqRecord::new(b"r2 lane".to_vec(), b"TGCARY".to_vec(), b"##JJJ$".to_vec())
            .expect("valid record"),
    ];

    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder()
        .inner(&mut buf)
        .two_bit_exact()
        .binned_quality()
        .split_names()
        .build();
    for record in &records {
        writer.write_record(record).expect("write should succeed");
    }
    writer.finish().expect("finish should succeed");

    let mut reader = DryIceReader::builder()
        .inner(buf.as_slice())
        .two_bit_exact()
        .quality_codec::<BinnedQualityCodec>()
        .name_codec::<SplitNameCodec>()
        .select(Sequence)
        .build()
        .expect("selected reader should build");

    let mut read_back = Vec::new();
    while let Some(record) = reader.next_record().expect("next_record should succeed") {
        read_back.push(record.sequence().to_vec());
    }

    let expected: Vec<Vec<u8>> = records.iter().map(|r| r.sequence().to_vec()).collect();
    assert_eq!(read_back, expected);
}

#[test]
fn selected_reader_compact_quality_only_scan() {
    let records = vec![
        SeqRecord::new(b"r1 desc".to_vec(), b"ACGTNN".to_vec(), b"!!III#".to_vec())
            .expect("valid record"),
        SeqRecord::new(b"r2 lane".to_vec(), b"TGCARY".to_vec(), b"##JJJ$".to_vec())
            .expect("valid record"),
    ];

    let expected: Vec<Vec<u8>> = records
        .iter()
        .map(|r| BinnedQualityCodec::encode(r.quality()).expect("binning should succeed"))
        .collect();

    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder()
        .inner(&mut buf)
        .two_bit_exact()
        .binned_quality()
        .split_names()
        .build();
    for record in &records {
        writer.write_record(record).expect("write should succeed");
    }
    writer.finish().expect("finish should succeed");

    let mut reader = DryIceReader::builder()
        .inner(buf.as_slice())
        .two_bit_exact()
        .quality_codec::<BinnedQualityCodec>()
        .name_codec::<SplitNameCodec>()
        .select(Quality)
        .build()
        .expect("selected reader should build");

    let mut read_back = Vec::new();
    while let Some(record) = reader.next_record().expect("next_record should succeed") {
        read_back.push(record.quality().to_vec());
    }

    assert_eq!(read_back, expected);
}

#[test]
fn selected_reader_compact_name_only_scan() {
    let records = vec![
        SeqRecord::new(b"r1 desc".to_vec(), b"ACGTNN".to_vec(), b"!!III#".to_vec())
            .expect("valid record"),
        SeqRecord::new(b"r2 lane".to_vec(), b"TGCARY".to_vec(), b"##JJJ$".to_vec())
            .expect("valid record"),
    ];

    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder()
        .inner(&mut buf)
        .two_bit_exact()
        .binned_quality()
        .split_names()
        .build();
    for record in &records {
        writer.write_record(record).expect("write should succeed");
    }
    writer.finish().expect("finish should succeed");

    let mut reader = DryIceReader::builder()
        .inner(buf.as_slice())
        .two_bit_exact()
        .quality_codec::<BinnedQualityCodec>()
        .name_codec::<SplitNameCodec>()
        .select(Name)
        .build()
        .expect("selected reader should build");

    let mut read_back = Vec::new();
    while let Some(record) = reader.next_record().expect("next_record should succeed") {
        read_back.push(record.name().to_vec());
    }

    let expected: Vec<Vec<u8>> = records.iter().map(|r| r.name().to_vec()).collect();
    assert_eq!(read_back, expected);
}

#[test]
fn selected_reader_compact_sequence_and_key_scan() {
    let records = vec![
        SeqRecord::new(b"r1 desc".to_vec(), b"ACGTNN".to_vec(), b"!!III#".to_vec())
            .expect("valid record"),
        SeqRecord::new(b"r2 lane".to_vec(), b"TGCARY".to_vec(), b"##JJJ$".to_vec())
            .expect("valid record"),
    ];
    let keys = vec![Bytes8Key(*b"key00001"), Bytes8Key(*b"key00002")];

    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder()
        .inner(&mut buf)
        .two_bit_exact()
        .binned_quality()
        .split_names()
        .bytes8_key()
        .build();
    for (record, key) in records.iter().zip(keys.iter()) {
        writer
            .write_record_with_key(record, key)
            .expect("write should succeed");
    }
    writer.finish().expect("finish should succeed");

    let mut reader = DryIceReader::builder()
        .inner(buf.as_slice())
        .two_bit_exact()
        .quality_codec::<BinnedQualityCodec>()
        .name_codec::<SplitNameCodec>()
        .bytes8_key()
        .select(Sequence | Key)
        .build()
        .expect("selected keyed reader should build");

    let mut read_sequences = Vec::new();
    let mut read_keys = Vec::new();
    while let Some(record) = reader.next_record().expect("next_record should succeed") {
        read_sequences.push(record.sequence().to_vec());
        read_keys.push(record.record_key().expect("key should decode"));
    }

    let expected_sequences: Vec<Vec<u8>> = records.iter().map(|r| r.sequence().to_vec()).collect();
    assert_eq!(read_sequences, expected_sequences);
    assert_eq!(read_keys, keys);
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
    let options = DryIceWriterOptions {
        layout: BlockLayoutOptions {
            block_size: BlockSizePolicy::TargetBytes(4096),
        },
    };

    let buf = Vec::new();
    let result = DryIceWriter::<_, RawAsciiCodec, RawQualityCodec, RawNameCodec, _>::from_options(
        buf, &options,
    );
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

    let mut reader = DryIceReader::with_two_bit_exact(buf.as_slice()).expect("reader should open");
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

#[test]
fn binned_quality_round_trip() {
    let records = vec![
        SeqRecord::new(b"r1".to_vec(), b"ACGT".to_vec(), b"!!!!".to_vec()).expect("valid record"),
        SeqRecord::new(b"r2".to_vec(), b"TGCA".to_vec(), b"IIII".to_vec()).expect("valid record"),
    ];

    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder()
        .inner(&mut buf)
        .binned_quality()
        .build();

    for record in &records {
        writer.write_record(record).expect("write should succeed");
    }
    writer.finish().expect("finish should succeed");

    let mut reader = DryIceReader::with_codecs::<
        dryice::RawAsciiCodec,
        dryice::BinnedQualityCodec,
        dryice::RawNameCodec,
    >(buf.as_slice())
    .expect("reader should open");
    let mut read_back = Vec::new();
    while reader.next_record().expect("next_record should succeed") {
        read_back.push(
            reader
                .to_seq_record()
                .expect("to_seq_record should succeed"),
        );
    }

    assert_eq!(read_back.len(), records.len());
    for (original, decoded) in records.iter().zip(read_back.iter()) {
        assert_eq!(original.name(), decoded.name());
        assert_eq!(original.sequence(), decoded.sequence());
        assert_eq!(
            original.quality().len(),
            decoded.quality().len(),
            "binned quality should preserve length"
        );
    }
}

#[test]
fn binned_quality_is_lossy() {
    let qual_in = vec![33 + 5, 33 + 15, 33 + 25, 33 + 35];
    let records = [
        SeqRecord::new(b"r1".to_vec(), b"ACGT".to_vec(), qual_in.clone()).expect("valid record"),
    ];

    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder()
        .inner(&mut buf)
        .binned_quality()
        .build();

    writer
        .write_record(&records[0])
        .expect("write should succeed");
    writer.finish().expect("finish should succeed");

    let mut reader = DryIceReader::with_codecs::<
        dryice::RawAsciiCodec,
        dryice::BinnedQualityCodec,
        dryice::RawNameCodec,
    >(buf.as_slice())
    .expect("reader should open");
    assert!(reader.next_record().expect("next_record should succeed"));

    let decoded_qual = reader.quality();
    assert_ne!(
        decoded_qual,
        qual_in.as_slice(),
        "binned quality should differ from original"
    );
    assert_eq!(decoded_qual.len(), qual_in.len());
}

#[test]
fn two_bit_exact_with_binned_quality_round_trip() {
    let records = [
        SeqRecord::new(b"r1".to_vec(), b"ACNGT".to_vec(), b"!!!!!".to_vec()).expect("valid record"),
    ];

    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder()
        .inner(&mut buf)
        .two_bit_exact()
        .binned_quality()
        .build();

    writer
        .write_record(&records[0])
        .expect("write should succeed");
    writer.finish().expect("finish should succeed");

    let mut reader = DryIceReader::with_codecs::<
        dryice::TwoBitExactCodec,
        dryice::BinnedQualityCodec,
        dryice::RawNameCodec,
    >(buf.as_slice())
    .expect("reader should open");
    assert!(reader.next_record().expect("next_record should succeed"));

    assert_eq!(reader.name(), b"r1");
    assert_eq!(reader.sequence(), b"ACNGT");
    assert_eq!(reader.quality().len(), 5);
}

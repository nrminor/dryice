//! Read throughput benchmarks across codec configurations.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use dryice::{
    BinnedQualityCodec, DryIceReader, DryIceWriter, SeqRecordLike, SplitNameCodec, TwoBitExactCodec,
};
use dryice_benchmarks::{compute_sort_key, generate_records};

const RECORD_COUNT: usize = 10_000;

fn prepare_raw_file(records: &[dryice::SeqRecord]) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder().inner(&mut buf).build();
    for record in records {
        writer.write_record(record).expect("write should succeed");
    }
    writer.finish().expect("finish should succeed");
    buf
}

fn prepare_compact_file(records: &[dryice::SeqRecord]) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder()
        .inner(&mut buf)
        .two_bit_exact()
        .binned_quality()
        .split_names()
        .build();
    for record in records {
        writer.write_record(record).expect("write should succeed");
    }
    writer.finish().expect("finish should succeed");
    buf
}

fn prepare_keyed_file(records: &[dryice::SeqRecord]) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder().inner(&mut buf).bytes8_key().build();
    for record in records {
        let key = compute_sort_key(record.sequence());
        writer
            .write_record_with_key(record, &key)
            .expect("write should succeed");
    }
    writer.finish().expect("finish should succeed");
    buf
}

fn bench_read(c: &mut Criterion) {
    let records = generate_records(RECORD_COUNT);
    let payload_size: usize = records
        .iter()
        .map(|r| r.name().len() + r.sequence().len() + r.quality().len())
        .sum();

    let raw_file = prepare_raw_file(&records);
    let compact_file = prepare_compact_file(&records);
    let keyed_file = prepare_keyed_file(&records);

    let mut group = c.benchmark_group("read_zero_copy");
    group.throughput(Throughput::Bytes(payload_size as u64));

    group.bench_function(BenchmarkId::new("raw", RECORD_COUNT), |b| {
        b.iter(|| {
            let mut reader = DryIceReader::new(raw_file.as_slice()).expect("reader should open");
            let mut count = 0u64;
            while reader.next_record().expect("next_record should succeed") {
                count += reader.sequence().len() as u64;
            }
            count
        });
    });

    group.bench_function(BenchmarkId::new("compact", RECORD_COUNT), |b| {
        b.iter(|| {
            let mut reader =
                DryIceReader::with_codecs::<TwoBitExactCodec, BinnedQualityCodec, SplitNameCodec>(
                    compact_file.as_slice(),
                )
                .expect("reader should open");
            let mut count = 0u64;
            while reader.next_record().expect("next_record should succeed") {
                count += reader.sequence().len() as u64;
            }
            count
        });
    });

    group.bench_function(BenchmarkId::new("keyed", RECORD_COUNT), |b| {
        b.iter(|| {
            let mut reader =
                DryIceReader::with_bytes8_key(keyed_file.as_slice()).expect("reader should open");
            let mut count = 0u64;
            while reader.next_record().expect("next_record should succeed") {
                let _key = reader.record_key().expect("key should decode");
                count += reader.sequence().len() as u64;
            }
            count
        });
    });

    group.finish();
}

criterion_group!(benches, bench_read);
criterion_main!(benches);

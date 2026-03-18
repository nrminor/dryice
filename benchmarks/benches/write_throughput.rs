//! Write throughput benchmarks across codec configurations.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use dryice::DryIceWriter;
use dryice_benchmarks::{compute_sort_key, generate_records};

const RECORD_COUNT: usize = 10_000;

fn bench_write_raw(c: &mut Criterion) {
    let records = generate_records(RECORD_COUNT);
    let payload_size: usize = records
        .iter()
        .map(|r| r.name().len() + r.sequence().len() + r.quality().len())
        .sum();

    let mut group = c.benchmark_group("write");
    group.throughput(Throughput::Bytes(payload_size as u64));

    group.bench_function(BenchmarkId::new("raw", RECORD_COUNT), |b| {
        b.iter(|| {
            let mut buf = Vec::with_capacity(payload_size * 2);
            let mut writer = DryIceWriter::builder().inner(&mut buf).build();
            for record in &records {
                writer.write_record(record).expect("write should succeed");
            }
            writer.finish().expect("finish should succeed");
        });
    });

    group.bench_function(BenchmarkId::new("two_bit_exact", RECORD_COUNT), |b| {
        b.iter(|| {
            let mut buf = Vec::with_capacity(payload_size);
            let mut writer = DryIceWriter::builder()
                .inner(&mut buf)
                .two_bit_exact()
                .build();
            for record in &records {
                writer.write_record(record).expect("write should succeed");
            }
            writer.finish().expect("finish should succeed");
        });
    });

    group.bench_function(BenchmarkId::new("compact", RECORD_COUNT), |b| {
        b.iter(|| {
            let mut buf = Vec::with_capacity(payload_size);
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
        });
    });

    group.bench_function(BenchmarkId::new("raw_with_key", RECORD_COUNT), |b| {
        let keys: Vec<_> = records
            .iter()
            .map(|r| compute_sort_key(r.sequence()))
            .collect();
        b.iter(|| {
            let mut buf = Vec::with_capacity(payload_size * 2);
            let mut writer = DryIceWriter::builder().inner(&mut buf).bytes8_key().build();
            for (record, key) in records.iter().zip(keys.iter()) {
                writer
                    .write_record_with_key(record, key)
                    .expect("write should succeed");
            }
            writer.finish().expect("finish should succeed");
        });
    });

    group.finish();
}

criterion_group!(benches, bench_write_raw);
criterion_main!(benches);

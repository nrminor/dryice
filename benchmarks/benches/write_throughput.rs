//! Write throughput benchmarks across formats and codec configurations.

use std::io::Write;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use dryice::DryIceWriter;
use dryice_benchmarks::{
    compute_sort_key, generate_records, payload_size, write_fastq, write_raw_binary,
};
use flate2::{Compression, write::GzEncoder};

const RECORD_COUNT: usize = 10_000;

fn bench_write(c: &mut Criterion) {
    let records = generate_records(RECORD_COUNT);
    let size = payload_size(&records);

    let mut group = c.benchmark_group("write");
    group.throughput(Throughput::Bytes(size as u64));

    group.bench_function(BenchmarkId::new("raw_binary", RECORD_COUNT), |b| {
        b.iter(|| {
            let mut buf = Vec::with_capacity(size * 2);
            write_raw_binary(&records, &mut buf);
            buf.len()
        });
    });

    group.bench_function(BenchmarkId::new("fastq", RECORD_COUNT), |b| {
        b.iter(|| {
            let mut buf = Vec::with_capacity(size * 2);
            write_fastq(&records, &mut buf);
            buf.len()
        });
    });

    group.bench_function(BenchmarkId::new("fastq_gzip", RECORD_COUNT), |b| {
        b.iter(|| {
            let buf = Vec::with_capacity(size);
            let mut gz = GzEncoder::new(buf, Compression::fast());
            let mut fastq = Vec::with_capacity(size * 2);
            write_fastq(&records, &mut fastq);
            gz.write_all(&fastq).expect("gzip write should succeed");
            gz.finish().expect("gzip finish should succeed").len()
        });
    });

    group.bench_function(BenchmarkId::new("dryice_raw", RECORD_COUNT), |b| {
        b.iter(|| {
            let mut buf = Vec::with_capacity(size * 2);
            let mut writer = DryIceWriter::builder().inner(&mut buf).build();
            for record in &records {
                writer.write_record(record).expect("write should succeed");
            }
            writer.finish().expect("finish should succeed");
        });
    });

    group.bench_function(
        BenchmarkId::new("dryice_two_bit_exact", RECORD_COUNT),
        |b| {
            b.iter(|| {
                let mut buf = Vec::with_capacity(size);
                let mut writer = DryIceWriter::builder()
                    .inner(&mut buf)
                    .two_bit_exact()
                    .build();
                for record in &records {
                    writer.write_record(record).expect("write should succeed");
                }
                writer.finish().expect("finish should succeed");
            });
        },
    );

    group.bench_function(BenchmarkId::new("dryice_compact", RECORD_COUNT), |b| {
        b.iter(|| {
            let mut buf = Vec::with_capacity(size);
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

    group.bench_function(BenchmarkId::new("dryice_raw_with_key", RECORD_COUNT), |b| {
        let keys: Vec<_> = records
            .iter()
            .map(|r| compute_sort_key(r.sequence()))
            .collect();
        b.iter(|| {
            let mut buf = Vec::with_capacity(size * 2);
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

criterion_group!(benches, bench_write);
criterion_main!(benches);

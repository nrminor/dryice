//! Round-trip (write + read) throughput benchmarks across formats.

use std::io::Read;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use dryice::{
    BinnedQualityCodec, DryIceReader, DryIceWriter, SeqRecordLike, SplitNameCodec, TwoBitExactCodec,
};
use dryice_benchmarks::{
    generate_records, payload_size, read_fastq, read_raw_binary, write_fastq, write_raw_binary,
};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};

const RECORD_COUNT: usize = 10_000;

fn bench_round_trip(c: &mut Criterion) {
    let records = generate_records(RECORD_COUNT);
    let size = payload_size(&records);

    let mut group = c.benchmark_group("round_trip");
    group.throughput(Throughput::Bytes(size as u64));

    group.bench_function(BenchmarkId::new("raw_binary", RECORD_COUNT), |b| {
        b.iter(|| {
            let mut buf = Vec::with_capacity(size * 2);
            write_raw_binary(&records, &mut buf);

            let mut count = 0u64;
            read_raw_binary(&buf, |_, seq, _| {
                count += seq.len() as u64;
            });
            count
        });
    });

    group.bench_function(BenchmarkId::new("fastq", RECORD_COUNT), |b| {
        b.iter(|| {
            let mut buf = Vec::with_capacity(size * 2);
            write_fastq(&records, &mut buf);

            let mut count = 0u64;
            read_fastq(&buf, |_, seq, _| {
                count += seq.len() as u64;
            });
            count
        });
    });

    group.bench_function(BenchmarkId::new("fastq_gzip", RECORD_COUNT), |b| {
        b.iter(|| {
            let mut fastq = Vec::with_capacity(size * 2);
            write_fastq(&records, &mut fastq);
            let compressed = Vec::with_capacity(size);
            let mut gz = GzEncoder::new(compressed, Compression::fast());
            std::io::Write::write_all(&mut gz, &fastq).expect("gzip write");
            let compressed = gz.finish().expect("gzip finish");

            let mut decompressed = Vec::new();
            GzDecoder::new(compressed.as_slice())
                .read_to_end(&mut decompressed)
                .expect("gzip decompress");
            let mut count = 0u64;
            read_fastq(&decompressed, |_, seq, _| {
                count += seq.len() as u64;
            });
            count
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

            let mut reader = DryIceReader::new(buf.as_slice()).expect("reader should open");
            let mut count = 0u64;
            while reader.next_record().expect("next_record should succeed") {
                count += reader.sequence().len() as u64;
            }
            count
        });
    });

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

            let mut reader =
                DryIceReader::with_codecs::<TwoBitExactCodec, BinnedQualityCodec, SplitNameCodec>(
                    buf.as_slice(),
                )
                .expect("reader should open");
            let mut count = 0u64;
            while reader.next_record().expect("next_record should succeed") {
                count += reader.sequence().len() as u64;
            }
            count
        });
    });

    group.finish();
}

criterion_group!(benches, bench_round_trip);
criterion_main!(benches);

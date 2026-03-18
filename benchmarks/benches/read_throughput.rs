//! Read throughput benchmarks across formats and codec configurations.

use std::io::Read;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use dryice::{
    BinnedQualityCodec, DryIceReader, DryIceWriter, SeqRecordLike, SplitNameCodec, TwoBitExactCodec,
};
use dryice_benchmarks::{
    compute_sort_key, generate_records, payload_size, read_fastq, read_raw_binary, write_fastq,
    write_raw_binary,
};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};

const RECORD_COUNT: usize = 10_000;

fn prepare_dryice_raw(records: &[dryice::SeqRecord]) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder().inner(&mut buf).build();
    for record in records {
        writer.write_record(record).expect("write should succeed");
    }
    writer.finish().expect("finish should succeed");
    buf
}

fn prepare_dryice_compact(records: &[dryice::SeqRecord]) -> Vec<u8> {
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

fn prepare_dryice_keyed(records: &[dryice::SeqRecord]) -> Vec<u8> {
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

fn prepare_fastq(records: &[dryice::SeqRecord]) -> Vec<u8> {
    let mut buf = Vec::new();
    write_fastq(records, &mut buf);
    buf
}

fn prepare_fastq_gzip(records: &[dryice::SeqRecord]) -> Vec<u8> {
    let mut fastq = Vec::new();
    write_fastq(records, &mut fastq);
    let buf = Vec::new();
    let mut gz = GzEncoder::new(buf, Compression::fast());
    std::io::Write::write_all(&mut gz, &fastq).expect("gzip write should succeed");
    gz.finish().expect("gzip finish should succeed")
}

fn prepare_raw_binary(records: &[dryice::SeqRecord]) -> Vec<u8> {
    let mut buf = Vec::new();
    write_raw_binary(records, &mut buf);
    buf
}

fn bench_read(c: &mut Criterion) {
    let records = generate_records(RECORD_COUNT);
    let size = payload_size(&records);

    let raw_binary_file = prepare_raw_binary(&records);
    let fastq_file = prepare_fastq(&records);
    let fastq_gzip_file = prepare_fastq_gzip(&records);
    let dryice_raw_file = prepare_dryice_raw(&records);
    let dryice_compact_file = prepare_dryice_compact(&records);
    let dryice_keyed_file = prepare_dryice_keyed(&records);

    let mut group = c.benchmark_group("read");
    group.throughput(Throughput::Bytes(size as u64));

    group.bench_function(BenchmarkId::new("raw_binary", RECORD_COUNT), |b| {
        b.iter(|| {
            let mut count = 0u64;
            read_raw_binary(&raw_binary_file, |_, seq, _| {
                count += seq.len() as u64;
            });
            count
        });
    });

    group.bench_function(BenchmarkId::new("fastq", RECORD_COUNT), |b| {
        b.iter(|| {
            let mut count = 0u64;
            read_fastq(&fastq_file, |_, seq, _| {
                count += seq.len() as u64;
            });
            count
        });
    });

    group.bench_function(BenchmarkId::new("fastq_gzip", RECORD_COUNT), |b| {
        b.iter(|| {
            let mut decompressed = Vec::new();
            GzDecoder::new(fastq_gzip_file.as_slice())
                .read_to_end(&mut decompressed)
                .expect("gzip decompress should succeed");
            let mut count = 0u64;
            read_fastq(&decompressed, |_, seq, _| {
                count += seq.len() as u64;
            });
            count
        });
    });

    group.bench_function(BenchmarkId::new("dryice_raw", RECORD_COUNT), |b| {
        b.iter(|| {
            let mut reader =
                DryIceReader::new(dryice_raw_file.as_slice()).expect("reader should open");
            let mut count = 0u64;
            while reader.next_record().expect("next_record should succeed") {
                count += reader.sequence().len() as u64;
            }
            count
        });
    });

    group.bench_function(BenchmarkId::new("dryice_compact", RECORD_COUNT), |b| {
        b.iter(|| {
            let mut reader =
                DryIceReader::with_codecs::<TwoBitExactCodec, BinnedQualityCodec, SplitNameCodec>(
                    dryice_compact_file.as_slice(),
                )
                .expect("reader should open");
            let mut count = 0u64;
            while reader.next_record().expect("next_record should succeed") {
                count += reader.sequence().len() as u64;
            }
            count
        });
    });

    group.bench_function(BenchmarkId::new("dryice_keyed", RECORD_COUNT), |b| {
        b.iter(|| {
            let mut reader = DryIceReader::with_bytes8_key(dryice_keyed_file.as_slice())
                .expect("reader should open");
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

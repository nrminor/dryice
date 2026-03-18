//! Round-trip (write + read) throughput benchmarks.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use dryice::{
    BinnedQualityCodec, DryIceReader, DryIceWriter, SeqRecordLike, SplitNameCodec, TwoBitExactCodec,
};
use dryice_benchmarks::generate_records;

const RECORD_COUNT: usize = 10_000;

fn bench_round_trip(c: &mut Criterion) {
    let records = generate_records(RECORD_COUNT);
    let payload_size: usize = records
        .iter()
        .map(|r| r.name().len() + r.sequence().len() + r.quality().len())
        .sum();

    let mut group = c.benchmark_group("round_trip");
    group.throughput(Throughput::Bytes(payload_size as u64));

    group.bench_function(BenchmarkId::new("raw", RECORD_COUNT), |b| {
        b.iter(|| {
            let mut buf = Vec::with_capacity(payload_size * 2);
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

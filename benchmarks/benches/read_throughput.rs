//! Read throughput benchmarks across formats and codec configurations.

use std::io::Read;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use dryice::{
    BinnedQualityCodec, DryIceReader, DryIceWriter, SeqRecordLike, SplitNameCodec,
    TwoBitExactCodec,
    fields::{Key, Name, Quality, Sequence},
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

fn prepare_dryice_two_bit_exact(records: &[dryice::SeqRecord]) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder()
        .inner(&mut buf)
        .two_bit_exact()
        .build();
    for record in records {
        writer.write_record(record).expect("write should succeed");
    }
    writer.finish().expect("finish should succeed");
    buf
}

fn prepare_dryice_binned_quality(records: &[dryice::SeqRecord]) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder()
        .inner(&mut buf)
        .binned_quality()
        .build();
    for record in records {
        writer.write_record(record).expect("write should succeed");
    }
    writer.finish().expect("finish should succeed");
    buf
}

fn prepare_dryice_split_names(records: &[dryice::SeqRecord]) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder()
        .inner(&mut buf)
        .split_names()
        .build();
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

fn prepare_dryice_compact_keyed(records: &[dryice::SeqRecord]) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut writer = DryIceWriter::builder()
        .inner(&mut buf)
        .two_bit_exact()
        .binned_quality()
        .split_names()
        .bytes8_key()
        .build();
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

struct ReadBenchInputs {
    size: usize,
    raw_binary_file: Vec<u8>,
    fastq_file: Vec<u8>,
    fastq_gzip_file: Vec<u8>,
    dryice_raw_file: Vec<u8>,
    dryice_two_bit_exact_file: Vec<u8>,
    dryice_binned_quality_file: Vec<u8>,
    dryice_split_names_file: Vec<u8>,
    dryice_compact_file: Vec<u8>,
    dryice_keyed_file: Vec<u8>,
    dryice_compact_keyed_file: Vec<u8>,
}

fn prepare_read_bench_inputs() -> ReadBenchInputs {
    let records = generate_records(RECORD_COUNT);
    ReadBenchInputs {
        size: payload_size(&records),
        raw_binary_file: prepare_raw_binary(&records),
        fastq_file: prepare_fastq(&records),
        fastq_gzip_file: prepare_fastq_gzip(&records),
        dryice_raw_file: prepare_dryice_raw(&records),
        dryice_two_bit_exact_file: prepare_dryice_two_bit_exact(&records),
        dryice_binned_quality_file: prepare_dryice_binned_quality(&records),
        dryice_split_names_file: prepare_dryice_split_names(&records),
        dryice_compact_file: prepare_dryice_compact(&records),
        dryice_keyed_file: prepare_dryice_keyed(&records),
        dryice_compact_keyed_file: prepare_dryice_compact_keyed(&records),
    }
}

fn bench_read_baselines(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    inputs: &ReadBenchInputs,
) {
    group.bench_function(BenchmarkId::new("raw_binary", RECORD_COUNT), |b| {
        b.iter(|| {
            let mut count = 0u64;
            read_raw_binary(&inputs.raw_binary_file, |_, seq, _| {
                count += seq.len() as u64;
            });
            count
        });
    });

    group.bench_function(BenchmarkId::new("fastq", RECORD_COUNT), |b| {
        b.iter(|| {
            let mut count = 0u64;
            read_fastq(&inputs.fastq_file, |_, seq, _| {
                count += seq.len() as u64;
            });
            count
        });
    });

    group.bench_function(BenchmarkId::new("fastq_gzip", RECORD_COUNT), |b| {
        b.iter(|| {
            let mut decompressed = Vec::new();
            GzDecoder::new(inputs.fastq_gzip_file.as_slice())
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
                DryIceReader::new(inputs.dryice_raw_file.as_slice()).expect("reader should open");
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
                    inputs.dryice_compact_file.as_slice(),
                )
                .expect("reader should open");
            let mut count = 0u64;
            while reader.next_record().expect("next_record should succeed") {
                count += reader.sequence().len() as u64;
            }
            count
        });
    });
}

fn bench_read_attribution(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    inputs: &ReadBenchInputs,
) {
    bench_read_codec_attribution(group, inputs);
    bench_read_compact_attribution(group, inputs);
}

fn bench_read_codec_attribution(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    inputs: &ReadBenchInputs,
) {
    group.bench_function(
        BenchmarkId::new("dryice_two_bit_exact_seq_only", RECORD_COUNT),
        |b| {
            b.iter(|| {
                let mut reader =
                    DryIceReader::with_two_bit_exact(inputs.dryice_two_bit_exact_file.as_slice())
                        .expect("reader should open");
                let mut count = 0u64;
                while reader.next_record().expect("next_record should succeed") {
                    count += reader.sequence().len() as u64;
                }
                count
            });
        },
    );

    group.bench_function(
        BenchmarkId::new("dryice_binned_quality_quality_only", RECORD_COUNT),
        |b| {
            b.iter(|| {
                let mut reader = DryIceReader::with_codecs::<
                    dryice::RawAsciiCodec,
                    BinnedQualityCodec,
                    dryice::RawNameCodec,
                >(inputs.dryice_binned_quality_file.as_slice())
                .expect("reader should open");
                let mut count = 0u64;
                while reader.next_record().expect("next_record should succeed") {
                    count += reader.quality().len() as u64;
                }
                count
            });
        },
    );

    group.bench_function(
        BenchmarkId::new("dryice_split_names_name_only", RECORD_COUNT),
        |b| {
            b.iter(|| {
                let mut reader = DryIceReader::with_codecs::<
                    dryice::RawAsciiCodec,
                    dryice::RawQualityCodec,
                    SplitNameCodec,
                >(inputs.dryice_split_names_file.as_slice())
                .expect("reader should open");
                let mut count = 0u64;
                while reader.next_record().expect("next_record should succeed") {
                    count += reader.name().len() as u64;
                }
                count
            });
        },
    );
}

fn bench_read_compact_attribution(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    inputs: &ReadBenchInputs,
) {
    group.bench_function(
        BenchmarkId::new("dryice_compact_next_only", RECORD_COUNT),
        |b| {
            b.iter(|| {
                let mut reader = DryIceReader::with_codecs::<
                    TwoBitExactCodec,
                    BinnedQualityCodec,
                    SplitNameCodec,
                >(inputs.dryice_compact_file.as_slice())
                .expect("reader should open");
                let mut count = 0u64;
                while reader.next_record().expect("next_record should succeed") {
                    count += 1;
                }
                count
            });
        },
    );

    group.bench_function(
        BenchmarkId::new("dryice_compact_all_fields", RECORD_COUNT),
        |b| {
            b.iter(|| {
                let mut reader = DryIceReader::with_codecs::<
                    TwoBitExactCodec,
                    BinnedQualityCodec,
                    SplitNameCodec,
                >(inputs.dryice_compact_file.as_slice())
                .expect("reader should open");
                let mut count = 0u64;
                while reader.next_record().expect("next_record should succeed") {
                    count += reader.name().len() as u64;
                    count += reader.sequence().len() as u64;
                    count += reader.quality().len() as u64;
                }
                count
            });
        },
    );

    group.bench_function(
        BenchmarkId::new("dryice_compact_to_owned", RECORD_COUNT),
        |b| {
            b.iter(|| {
                let reader = DryIceReader::with_codecs::<
                    TwoBitExactCodec,
                    BinnedQualityCodec,
                    SplitNameCodec,
                >(inputs.dryice_compact_file.as_slice())
                .expect("reader should open");
                let records = reader
                    .into_records()
                    .collect::<Result<Vec<_>, _>>()
                    .expect("records should decode");
                records.len() as u64
            });
        },
    );
}

fn bench_read_selected(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    inputs: &ReadBenchInputs,
) {
    group.bench_function(
        BenchmarkId::new("dryice_compact_selected_seq_only", RECORD_COUNT),
        |b| {
            b.iter(|| {
                let mut reader = DryIceReader::builder()
                    .inner(inputs.dryice_compact_file.as_slice())
                    .two_bit_exact()
                    .quality_codec::<BinnedQualityCodec>()
                    .name_codec::<SplitNameCodec>()
                    .select(Sequence)
                    .build()
                    .expect("selected reader should open");
                let mut count = 0u64;
                while let Some(record) = reader.next_record().expect("next_record should succeed") {
                    count += record.sequence().len() as u64;
                }
                count
            });
        },
    );

    group.bench_function(
        BenchmarkId::new("dryice_compact_selected_quality_only", RECORD_COUNT),
        |b| {
            b.iter(|| {
                let mut reader = DryIceReader::builder()
                    .inner(inputs.dryice_compact_file.as_slice())
                    .two_bit_exact()
                    .quality_codec::<BinnedQualityCodec>()
                    .name_codec::<SplitNameCodec>()
                    .select(Quality)
                    .build()
                    .expect("selected reader should open");
                let mut count = 0u64;
                while let Some(record) = reader.next_record().expect("next_record should succeed") {
                    count += record.quality().len() as u64;
                }
                count
            });
        },
    );

    group.bench_function(
        BenchmarkId::new("dryice_compact_selected_name_only", RECORD_COUNT),
        |b| {
            b.iter(|| {
                let mut reader = DryIceReader::builder()
                    .inner(inputs.dryice_compact_file.as_slice())
                    .two_bit_exact()
                    .quality_codec::<BinnedQualityCodec>()
                    .name_codec::<SplitNameCodec>()
                    .select(Name)
                    .build()
                    .expect("selected reader should open");
                let mut count = 0u64;
                while let Some(record) = reader.next_record().expect("next_record should succeed") {
                    count += record.name().len() as u64;
                }
                count
            });
        },
    );

    group.bench_function(
        BenchmarkId::new("dryice_compact_selected_seq_key", RECORD_COUNT),
        |b| {
            b.iter(|| {
                let mut reader = DryIceReader::builder()
                    .inner(inputs.dryice_compact_keyed_file.as_slice())
                    .two_bit_exact()
                    .quality_codec::<BinnedQualityCodec>()
                    .name_codec::<SplitNameCodec>()
                    .bytes8_key()
                    .select(Sequence | Key)
                    .build()
                    .expect("selected keyed reader should open");
                let mut count = 0u64;
                while let Some(record) = reader.next_record().expect("next_record should succeed") {
                    count += record.sequence().len() as u64;
                    let key = record.record_key().expect("key should decode");
                    count += key.0.len() as u64;
                }
                count
            });
        },
    );
}

fn bench_read_keyed(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    inputs: &ReadBenchInputs,
) {
    group.bench_function(BenchmarkId::new("dryice_keyed", RECORD_COUNT), |b| {
        b.iter(|| {
            let mut reader = DryIceReader::with_bytes8_key(inputs.dryice_keyed_file.as_slice())
                .expect("reader should open");
            let mut count = 0u64;
            while reader.next_record().expect("next_record should succeed") {
                let _key = reader.record_key().expect("key should decode");
                count += reader.sequence().len() as u64;
            }
            count
        });
    });

    group.bench_function(
        BenchmarkId::new("dryice_keyed_key_only", RECORD_COUNT),
        |b| {
            b.iter(|| {
                let mut reader = DryIceReader::with_bytes8_key(inputs.dryice_keyed_file.as_slice())
                    .expect("reader should open");
                let mut count = 0u64;
                while reader.next_record().expect("next_record should succeed") {
                    let key = reader.record_key().expect("key should decode");
                    count += key.0.len() as u64;
                }
                count
            });
        },
    );
}

fn bench_read(c: &mut Criterion) {
    let inputs = prepare_read_bench_inputs();

    let mut group = c.benchmark_group("read");
    group.throughput(Throughput::Bytes(inputs.size as u64));

    bench_read_baselines(&mut group, &inputs);
    bench_read_attribution(&mut group, &inputs);
    bench_read_selected(&mut group, &inputs);
    bench_read_keyed(&mut group, &inputs);

    group.finish();
}

criterion_group!(benches, bench_read);
criterion_main!(benches);

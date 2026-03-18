# dryice benchmarks

This crate measures `dryice` throughput across codec configurations and compares it against common alternative formats for temporary genomic data persistence. The benchmarks use [Criterion](https://github.com/bheisler/criterion.rs) with synthetic but realistically-sized Illumina-like records (150bp sequences, ~1% ambiguity rate, realistic quality distributions, instrument-style names).

## Running the benchmarks

Run all benchmarks:

```sh
cargo bench -p dryice-benchmarks
```

Run a specific benchmark suite:

```sh
cargo bench -p dryice-benchmarks --bench write_throughput
cargo bench -p dryice-benchmarks --bench read_throughput
cargo bench -p dryice-benchmarks --bench round_trip
```

For a quick sanity check (fewer iterations, less precise):

```sh
cargo bench -p dryice-benchmarks -- --quick
```

Criterion generates HTML reports in `target/criterion/`. Open `target/criterion/report/index.html` for interactive plots.

## Benchmark suites

There are three benchmark suites, each comparing the same set of formats.

**write_throughput** measures how fast each format can serialize 10,000 records from memory into a byte buffer. This isolates encoding overhead.

**read_throughput** measures how fast each format can deserialize pre-written data back into accessible record fields. For `dryice`, this uses the zero-copy `next_record()` path. For FASTQ, this uses a minimal line-based scanner (not a production parser).

**round_trip** measures the combined write-then-read cycle, which is the most representative metric for spill/reload workflows.

## Formats compared

| Format               | Description                                                                                                                                    |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| raw binary           | Length-prefixed binary dump of name + sequence + quality bytes. Theoretical throughput ceiling — no structure, no indexing, no codec overhead. |
| FASTQ                | Standard four-line text format with manual formatting and minimal line-based scanning.                                                         |
| gzip FASTQ           | FASTQ compressed with `flate2` at fast compression level.                                                                                      |
| dryice raw           | `dryice` with `RawAsciiCodec` + `RawQualityCodec` + `RawNameCodec`. The speed-first configuration.                                             |
| dryice two-bit exact | `dryice` with `TwoBitExactCodec` for sequences, raw quality and names.                                                                         |
| dryice compact       | `dryice` with `TwoBitExactCodec` + `BinnedQualityCodec` + `SplitNameCodec`. Full compact configuration.                                        |
| dryice raw + key     | `dryice` raw codecs with an 8-byte `Bytes8Key` record key.                                                                                     |

## Early results

These numbers are from a single machine (Apple M-series, `target-cpu=native`) and should be treated as directional rather than definitive. Your results will vary with hardware, OS, and memory pressure.

### Write throughput

| Format                   | Throughput     |
| ------------------------ | -------------- |
| raw binary               | 32.9 GiB/s     |
| FASTQ                    | 31.4 GiB/s     |
| **dryice raw**           | **15.7 GiB/s** |
| gzip FASTQ               | 1.8 GiB/s      |
| **dryice two-bit exact** | **1.8 GiB/s**  |
| **dryice compact**       | **880 MiB/s**  |

### Read throughput

| Format             | Throughput     |
| ------------------ | -------------- |
| raw binary         | 31.8 GiB/s     |
| **dryice raw**     | **29.0 GiB/s** |
| **dryice keyed**   | **28.1 GiB/s** |
| **dryice compact** | **3.5 GiB/s**  |
| FASTQ              | 3.1 GiB/s      |
| gzip FASTQ         | 1.2 GiB/s      |

### Round-trip throughput

| Format             | Throughput     |
| ------------------ | -------------- |
| raw binary         | 16.3 GiB/s     |
| **dryice raw**     | **10.0 GiB/s** |
| FASTQ              | 3.0 GiB/s      |
| **dryice compact** | **699 MiB/s**  |
| gzip FASTQ         | 696 MiB/s      |

### What the numbers mean

The raw binary baseline represents the theoretical throughput ceiling: just copying bytes with length prefixes, no structure, no indexing, no codec overhead. Everything else is measured against that ceiling.

dryice raw read throughput is now within 10% of the raw binary ceiling (29.0 vs 31.8 GiB/s) thanks to identity codec optimization that returns slices directly into block payload bytes with zero copying. On round-trip, dryice raw is over 3x faster than FASTQ text (10.0 vs 3.0 GiB/s) while providing structured block-oriented access, zero-copy reads, optional record keys, and a self-describing format.

dryice compact mode trades throughput for a smaller footprint. It is comparable to gzip FASTQ in round-trip speed but provides random access within blocks and structured record fields rather than requiring full decompression before any record can be accessed.

Record keys add negligible overhead to both write and read paths — the key section is a simple fixed-width append/read alongside the existing payloads.

## Future work

These benchmarks currently use in-memory buffers, which isolates format overhead from I/O system performance. Future additions will include:

- benchmarks against real FASTQ parsers (e.g. `needletail`, `noodles-fastq`)
- file-backed benchmarks that measure actual disk I/O
- larger record counts to stress block boundary behavior
- long-read (nanopore-scale) record benchmarks

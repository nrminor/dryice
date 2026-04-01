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

There are three benchmark suites.

**write_throughput** measures how fast each format can serialize 10,000 records from memory into a byte buffer. This isolates encoding overhead. The write suite includes `dryice two-bit exact` explicitly so we can see the cost of sequence re-encoding alone before layering on binned qualities and split names.

**read_throughput** measures how fast each format can deserialize pre-written data back into accessible record fields. For `dryice`, this uses the `next_record()` path and includes a small attribution-oriented submatrix so we can separate codec costs from access-pattern costs and selective decoding behavior. For FASTQ, this uses a minimal line-based scanner (not a production parser).

**round_trip** measures the combined write-then-read cycle, which is the most representative metric for spill/reload workflows. The round-trip suite uses `dryice compact` as the full-feature representative configuration rather than enumerating every intermediate combination.

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

### Read attribution cases

The read suite keeps the headline comparisons above, but also adds a small set of dryice-specific diagnostic cases:

- `dryice_two_bit_exact_seq_only` isolates sequence decode cost without name/quality codec work
- `dryice_binned_quality_quality_only` isolates quality decode/access cost
- `dryice_split_names_name_only` isolates split-name decode/access cost
- `dryice_compact_next_only` measures block/index traversal with no field access
- `dryice_compact_all_fields` measures compact-mode read throughput when all borrowed fields are touched
- `dryice_compact_to_owned` measures the fully materialized owned-record path
- `dryice_compact_selected_seq_only` measures compact-mode sequence scans with selective decoding enabled
- `dryice_compact_selected_quality_only` measures compact-mode quality-only scans with selective decoding enabled
- `dryice_compact_selected_name_only` measures compact-mode name-only scans with selective decoding enabled
- `dryice_compact_selected_seq_key` measures compact-mode sequence-plus-key scans with selective decoding enabled
- `dryice_keyed_key_only` isolates record-key access overhead from field access

Importantly, the selective-decoding cases are not measuring partial file I/O or predicate pushdown. `dryice` still reads whole blocks. What changes is which fields are decoded into the current row projection. That distinction matters because because decoding, particularly for bitpacked sequences, can be expensive. Selective decoding means data can be stored in a compact form while still supporting fast intermediate passes when a stage of the user's algorithm only needs part of each record, and thus only needs to pay the expense of decoding that part. More on this below!

## Early results

These numbers are from a single machine (Apple M-series, `target-cpu=native`) and should be treated as directional rather than definitive. Your results will vary with hardware, OS, and memory pressure.

### Write throughput

| Format                   | Throughput     |
| ------------------------ | -------------- |
| raw binary               | 33.4 GiB/s     |
| FASTQ                    | 31.8 GiB/s     |
| **dryice raw**           | **15.3 GiB/s** |
| **dryice raw + key**     | **14.6 GiB/s** |
| gzip FASTQ               | 1.82 GiB/s     |
| **dryice two-bit exact** | **1.77 GiB/s** |
| **dryice compact**       | **876 MiB/s**  |

### Read throughput

| Format             | Throughput     |
| ------------------ | -------------- |
| raw binary         | 31.6 GiB/s     |
| **dryice raw**     | **29.2 GiB/s** |
| **dryice keyed**   | **27.5 GiB/s** |
| **dryice compact** | **3.34 GiB/s** |
| FASTQ              | 3.32 GiB/s     |
| gzip FASTQ         | 1.23 GiB/s     |

### Read throughput: selective decoding cases

| Format / case                             | Throughput     |
| ----------------------------------------- | -------------- |
| **dryice compact**                        | **3.4 GiB/s**  |
| **dryice compact, only decoding seq**     | **8.6 GiB/s**  |
| **dryice compact, only decoding quality** | **27.1 GiB/s** |
| **dryice compact, only decoding name**    | **5.7 GiB/s**  |
| **dryice compact, decoding seq + key**    | **6.0 GiB/s**  |

### Round-trip throughput

| Format             | Throughput     |
| ------------------ | -------------- |
| raw binary         | 16.1 GiB/s     |
| **dryice raw**     | **10.0 GiB/s** |
| FASTQ              | 2.85 GiB/s     |
| **dryice compact** | **707 MiB/s**  |
| gzip FASTQ         | 702 MiB/s      |

### What the numbers mean

The raw binary baseline represents the theoretical throughput ceiling: just copying bytes with length prefixes, no structure, no indexing, no codec overhead. Everything else is measured against that ceiling.

`dryice raw` is fast because most fields are effectively just memcpy. Names, sequences, and qualities are stored as bytes, and after the identity-codec optimization the reader can return slices directly into block payload bytes with zero copying. That is why raw read throughput is now within 10% of the raw binary ceiling (29.2 vs 31.6 GiB/s), and raw round-trip throughput is over 3x faster than FASTQ text (10.0 vs 2.85 GiB/s) while still providing structured block-oriented access, zero-copy reads, optional record keys, and a self-describing format.

`dryice compact` is slower because it does real codec work. On writes, compact mode re-encodes sequences from raw bytes into packed 2-bit form with an ambiguity sideband, bins qualities into coarser Phred levels, and splits names into identifier/description form. On reads, full-row compact scans still prepare names, sequences, and qualities before exposing each record. That extra work is exactly what the baseline compact read benchmark is measuring.

Selective decoding changes that tradeoff materially. `dryice` still reads full blocks from the file, but it can now decode only the fields needed for the current pass. That means users can choose a compact, lossless storage configuration up front, then recover much of the read-side throughput during intermediate stages of an algorithm by decoding only the projection they actually need. In the current benchmarks, compact sequence-only scans with selective decoding are much faster than full compact reads, and the name-only, quality-only, and sequence-plus-key cases show that the benefit applies to multiple realistic projections rather than only one especially favorable path.

The quality-only case deserves especially careful interpretation. It is fast in large part because `BinnedQualityCodec` does its real transformation work on write; on read, decoding the binned representation is close to copying already-binned bytes back out. That does not make the result uninteresting. It means that if a workflow is willing to pay the upfront binning cost, then intermediate spill/reload stages that only need qualities can be very fast while still benefiting from a compact block format and low, steady memory usage. That is closely aligned with the broader goal of laptop-friendly bioinformatics workflows: compact temporary storage, constant-ish memory usage, and fast enough intermediate passes to keep the format practical in everyday algorithmic pipelines.

The benchmark matrix is intentionally uneven in a few places. `dryice two-bit exact` appears in write throughput so we can isolate the cost of sequence re-encoding alone before layering on quality and name codecs. The read suite now adds a few diagnostic dryice-only cases so we can attribute costs more honestly instead of bundling every codec and access pattern into one `compact` number. `dryice compact` is still the full-feature representative configuration for the round-trip benchmark.

Record keys add negligible overhead to both write and read paths — the key section is a simple fixed-width append/read alongside the existing payloads.

## Future work

These benchmarks currently use in-memory buffers, which isolates format overhead from I/O system performance. Future additions will include:

- benchmarks against real FASTQ parsers (e.g. `needletail`, `noodles-fastq`)
- file-backed benchmarks that measure actual disk I/O
- larger record counts to stress block boundary behavior
- long-read (nanopore-scale) record benchmarks

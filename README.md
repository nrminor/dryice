# DryIce: A Fast Temporary Data Storage Engine Tuned for Big Genomic Data

> _Don't put it in the minus-80—just put it on dry ice!_

> [!WARNING]  
> Though this repo is public, it's a work in progress and not ready for use yet!

## Overview

DryIce is a disk storage engine and file format optimized for temporary genomic data. Its purpose is to leverage extremely fast movement of sequence data on and off of disk to make larger-than-memory workloads more tractable. Want to do parallel comparisons between kmer hashes across cores? Want to do disk-spilling global sequence sorting? Want to cheaply scan for sequence matches across partitioned reads? DryIce is meant for these and more use cases.

> [!NOTE]
> DryIce is emphatically _not_ an archival genomics file format, a replacement for BAM or FASTQ, or a general-purpose columnar analytics format. Its niche is quick _temporary files_, where FASTQ, BAM, and text data formats may be ill-suited for rapid I/O, parsing, and searching.

Though optimized for I/O, DryIce also provides varying levels of compactness, allowing users to sacrifice read/write throughput for the benefit of a reduced disk footprint. Its architecture is also extensible; Rust library users can provide their own sequence encoding, quality score encoding, sequence ID/description encoding, and sequence record key encoding (more on record keys below!). These user-provided implementations can live alongside the implementations provided out-of-the-box in the `dryice` crate.

## Getting Started

> [!NOTE]
> Publishing to crates.io is coming soon. For now, install from source.

To add `dryice` as a dependency once it's published:

```sh
cargo add dryice
```

To build from source:

```sh
git clone https://github.com/nrminor/dryice.git
cd dryice
just check  # runs fmt, clippy, tests, and doc checks
```

## The `.dryice` File Format

DryIce organizes data onto disk with two priorities: 1) make I/O fast, and 2) use a batched, data-oriented design to get out of the CPU's way once data is in-memory. In line with these priorities, `.dryice` files feature a rich header followed by data blocks, each of which carry their own metadata, including offsets for accessing each sequence "record", as well as payloads of contiguous bytes. As mentioned, these bytes can be encoded/decoded with a variety of out-of-the-box codecs as well as user-defined codecs--DryIce is your oyster!

One unique feature of `.dryice` data is that it can store arrays of unique keys associated with each record in the payload. These keys can be used for sorting, filtering, searching, etc. without needing each record's sequence or quality score data itself. For an application like global FASTQ sorting, this means records can be sorted purely with their record keys rather than comparing whole sequences or generating kmers on the fly.
Below is an ASCII diagram of the file format followed by descriptions of each section.

```text
+==============================================================+
| FILE HEADER (8 bytes)                                        |
|--------------------------------------------------------------|
| magic: DRYI (4 bytes)                                        |
| version_major: u16 le                                        |
| version_minor: u16 le                                        |
+==============================================================+

+==============================================================+
| BLOCK 0                                                      |
|--------------------------------------------------------------|
| BLOCK HEADER (152 bytes)                                     |
|   record_count: u32 le                                       |
|   sequence_codec_tag: [u8; 16]                               |
|   quality_codec_tag:  [u8; 16]                               |
|   name_codec_tag:     [u8; 16]                               |
|   has_record_key:     u8                                     |
|   reserved:           u8                                     |
|   record_key_width:   u16 le                                 |
|   record_key_tag:     [u8; 16]                               |
|   index_range:        { offset: u64 le, len: u64 le }        |
|   names_range:        { offset: u64 le, len: u64 le }        |
|   sequences_range:    { offset: u64 le, len: u64 le }        |
|   qualities_range:    { offset: u64 le, len: u64 le }        |
|   record_keys_range:  { offset: u64 le, len: u64 le }        |
|--------------------------------------------------------------|
| RECORD INDEX                                                 |
|   entry 0: name_off name_len seq_off seq_len qual_off qual_l |
|   entry 1: ...                                               |
|   entry N: ...                                               |
|--------------------------------------------------------------|
| NAMES PAYLOAD (optional)                                     |
|   [encoded name bytes for all records in this block]         |
|--------------------------------------------------------------|
| SEQUENCES PAYLOAD                                            |
|   [encoded sequence bytes for all records in this block]     |
|--------------------------------------------------------------|
| QUALITIES PAYLOAD (optional)                                 |
|   [encoded quality bytes for all records in this block]      |
|--------------------------------------------------------------|
| RECORD KEYS PAYLOAD (optional)                               |
|   [fixed-width key bytes for all records in this block]      |
+==============================================================+

+==============================================================+
| BLOCK 1                                                      |
|   ...                                                        |
+==============================================================+

+==============================================================+
| BLOCK K                                                      |
|   ...                                                        |
+==============================================================+
```

### The File Header

Every `.dryice` file begins with an 8-byte header: a 4-byte magic number (`DRYI`) followed by major and minor version numbers as little-endian `u16` values. The reader validates the magic bytes and rejects files with an unsupported major version, providing a clean upgrade path for future format evolution without breaking older readers on compatible changes.

### Data Blocks

The body of a `.dryice` file is a sequence of self-contained blocks, each holding a batch of sequencing records. Blocks are the unit of I/O: the writer assembles records into a block until a configurable size threshold is reached, then flushes the entire block to disk. The reader loads one block at a time and yields records from it.

Each block is self-describing. Its header carries the codec type tags for sequence, quality, and name encodings, so the reader can verify that the file was written with the codecs it expects. This means different blocks in the same file could theoretically use different codecs, though in practice the writer uses the same codecs throughout.

#### Block Header

The block header is 152 bytes and contains two kinds of information: semantic metadata (what codecs were used, how many records, whether record keys are present) and layout metadata (byte ranges for each section within the block). The codec identities are stored as 16-byte type tags rather than small integer enum values, which means user-defined codecs are first-class citizens in the format — the header stores whatever tag the codec declares, and the reader verifies it matches at load time.

The five section ranges (index, names, sequences, qualities, record keys) use `{ offset, len }` pairs relative to the start of the block's payload area. Optional sections like names, qualities, and record keys use zero-length ranges when absent, and the reader determines presence from the codec tags and key metadata.

#### Record Index

Each block contains a fixed-width record index with one 24-byte entry per record. Each entry stores byte offsets and lengths into the block's payload sections for that record's name, sequence, and quality data. This gives constant-time access to any record's fields within the block without scanning the payload data, and it allows the reader to skip fields it doesn't need.

#### Record Keys

Record keys are optional fixed-width accelerator values stored in a dense array alongside the record payloads. They are designed for workflows where records need to be compared or ordered by a derived value — such as a minimizer hash, canonical k-mer, or partition identifier — without touching the full sequence or quality data.

The key system is trait-based: the `RecordKey` trait defines the width, type tag, and encode/decode behavior, and users can implement their own key types. The writer stores the key's type tag and width in the block header, and the reader verifies them at load time. Built-in key types (`Bytes8Key` and `Bytes16Key`) are provided for common use cases.

For external sorting, this means the merge phase can compare 8-byte or 16-byte keys in a min-heap without ever touching the sequence payloads — a major performance advantage over approaches that require reparsing or recomputing sort criteria on every comparison.

## Library Architecture

The `dryice` Rust library is designed around three core principles: parser independence, trait-based extensibility, and zero-copy reading.

The write-side boundary is the `SeqRecordLike` trait. Any type that can provide borrowed byte slices for name, sequence, and quality fields can be written into a `dryice` file without conversion into a crate-owned type. This means users of `noodles`, `needletail`, or any other Rust sequencing library can implement `SeqRecordLike` for their record types and write them directly.

The read-side boundary is the `DryIceReader` itself, which implements `SeqRecordLike` for the current record. After calling `next_record()`, the reader's `name()`, `sequence()`, and `quality()` methods return borrowed slices into block-owned buffers with no per-record heap allocation. For users who prefer iterator ergonomics, `into_records()` provides a standard `Iterator` that yields owned `SeqRecord` values.

Sequence, quality, and name encodings are selected via type parameters on the writer and reader, with sensible defaults. The writer builder uses typestate transitions to configure codecs and record keys:

```rust
let writer = DryIceWriter::builder()
    .inner(file)
    .two_bit_exact()
    .binned_quality()
    .split_names()
    .bytes8_key()
    .target_block_records(4096)
    .build();
```

All codec and key traits are public, so users can implement their own encodings and key types. The type system ensures that readers and writers always have a real codec implementation behind them, and codec mismatches between writer and reader are caught at block-load time with clear error messages.

### Built-in Codecs

DryIce ships with built-in codecs for all three record fields, plus built-in record key types. Users can also implement their own by implementing the `SequenceCodec`, `QualityCodec`, `NameCodec`, or `RecordKey` traits.

#### Sequence Codecs

| Codec               | Type     | Description                                                                                                                                                                      |
| ------------------- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `RawAsciiCodec`     | Lossless | Stores sequences as raw ASCII bytes. Fastest encode/decode, largest footprint.                                                                                                   |
| `TwoBitExactCodec`  | Lossless | Packs canonical bases (A/C/G/T) into 2 bits each via SIMD-accelerated `bitnuc`, with a sparse ambiguity sideband that preserves exact IUPAC symbols. Compact and exact.          |
| `TwoBitLossyNCodec` | Lossy    | Same 2-bit packing as `TwoBitExactCodec`, but collapses all ambiguous bases to `N`. More compact sideband than exact mode since only positions are stored, not original symbols. |

#### Quality Codecs

| Codec                 | Type     | Description                                                                                                                                                   |
| --------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `RawQualityCodec`     | Lossless | Stores quality scores as raw Phred+33 ASCII bytes.                                                                                                            |
| `BinnedQualityCodec`  | Lossy    | Illumina-style 8-level Phred binning. Reduces entropy for better downstream compression while preserving the most important quality distinctions. Idempotent. |
| `OmittedQualityCodec` | Lossy    | Drops quality scores entirely. Useful for workflows where quality information is not needed in the temporary representation.                                  |

#### Name Codecs

| Codec              | Type     | Description                                                                                                                                                   |
| ------------------ | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `RawNameCodec`     | Lossless | Stores the full name bytes as-is.                                                                                                                             |
| `SplitNameCodec`   | Lossless | Splits names on the first space into identifier and description, storing both with a length prefix. Decoded names carry parsed `id` and `description` fields. |
| `OmittedNameCodec` | Lossy    | Drops names entirely. Useful for workflows where record identity is tracked by position or key rather than by name.                                           |

#### Record Keys

| Key Type     | Width        | Description                                                                             |
| ------------ | ------------ | --------------------------------------------------------------------------------------- |
| `Bytes8Key`  | 8 bytes      | General-purpose 8-byte fixed-width key.                                                 |
| `Bytes16Key` | 16 bytes     | General-purpose 16-byte fixed-width key.                                                |
| Custom       | User-defined | Implement the `RecordKey` trait with your own width, type tag, and encode/decode logic. |

## Examples

The [`dryice/examples/`](dryice/examples/) directory contains standalone programs demonstrating the primary workflows `dryice` is designed for. Run any example with `cargo run --example <name>`.

- [**spill_reload**](dryice/examples/spill_reload.rs) — The most fundamental `dryice` pattern: spilling a batch of sequencing records to a temporary buffer and reloading them, demonstrating the building block for any out-of-core workflow.

- [**external_merge_sort**](dryice/examples/external_merge_sort.rs) — A complete external k-way merge sort that spills sorted runs with precomputed 8-byte record keys, then merges them using a min-heap that compares only the keys without touching sequence payloads.

- [**partitioning**](dryice/examples/partitioning.rs) — Partitioning records into separate temporary buckets based on a derived criterion, showing how `dryice` can serve as fast backing storage for partitioning stages in larger pipelines.

- [**compact_codecs**](dryice/examples/compact_codecs.rs) — Comparing raw versus compact storage using `TwoBitExactCodec`, `BinnedQualityCodec`, and `SplitNameCodec`, with size ratios and round-trip verification.

- [**record_keys**](dryice/examples/record_keys.rs) — Writing and reading records with fixed-width accelerator keys, demonstrating how keys are stored, retrieved, and associated with records through the type system.

- [**zero_copy_pipe**](dryice/examples/zero_copy_pipe.rs) — Piping records from one `dryice` file to another with no per-record allocation, using the fact that `DryIceReader` implements `SeqRecordLike`.

- [**custom_codec**](dryice/examples/custom_codec.rs) — Implementing a custom `SequenceCodec` (a simple run-length encoder), showing the full codec trait contract.

See the [examples README](dryice/examples/README.md) for more detailed descriptions of each example.

## Language Wrappers

Libraries for Python and Node (TypeScript/JavaScript) are coming soon!

## Citation

Also coming soon!

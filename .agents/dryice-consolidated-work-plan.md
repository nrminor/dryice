# `dryice` consolidated work plan

## Purpose of this document

This document consolidates the current stable conclusions from the earlier planning artifacts and the implementation work done so far into one implementation-oriented plan.

It is not meant to replace the earlier documents. It is meant to answer a narrower question:

```text
Given what we now know and what we have built,
what should we build next,
in what order,
and with what constraints in mind?
```

The current planning stack this document consolidates is:

- `.agents/dryice-design-first-pass.md`
- `.agents/dryice-rust-architecture-plan.md`
- `.agents/dryice-rust-design-notebook.md`

## Current project definition

`dryice` is a high-throughput transient container for read-like genomic records.

Its purpose is to make it cheap to move large collections of sequencing records onto disk and back again in temporary workflows where RAM is bounded and throughput matters more than interoperability. The flagship use case remains external sorting of FASTQ-like records, but the design should also support nearby out-of-core genomics workflows such as partitioning, regrouping, spill-backed staging, and other tasks that benefit from fast temporary persistence.

`dryice` is not intended to be:

- a new archival genomics file format
- a generic genomics transformation layer
- a parser-coupled library bound to one Rust bioinformatics ecosystem

## What has been built so far

The following phases are complete.

### Phase 1: repo and crate skeleton (done)

- workspace root with resolver 3, shared lints, and shared profiles
- single `dryice` library crate with `thiserror` and `bon` as dependencies, `proptest` as a dev dependency
- `justfile` with the standard check/fmt/lint/test/doc workflow
- deny-by-default `.gitignore`
- `AGENTS.md` philosophical primer
- CI via GitHub Actions running `just check`

### Phase 2: public surface skeleton (done)

- `SeqRecordLike` trait with `name()`, `sequence()`, `quality()`, `len()`, `is_empty()` default methods
- `SeqRecordExt` blanket extension trait providing `to_seq_record()`
- `SeqRecord` owned row-wise record type with private fields, invariant-preserving constructors, and accessors
- `DryIceError` with categorized error variants
- public encoding/config enums: `SequenceEncoding`, `QualityEncoding`, `NameEncoding`, `BlockSizePolicy`, `SortKeyKind`

### Phase 3: config/builder layer (done)

- `DryIceWriterOptions` with grouped sub-structs `EncodingOptions` and `BlockLayoutOptions`
- `bon`-derived flat builder on `DryIceWriter` using function-level `#[bon::builder]`
- `Default` impls on codec enums for clean config defaults
- `from_options()` escape hatch that rejects unsupported `TargetBytes` policy

### Phase 4: block schema and internal machinery (done)

- private `BlockHeader` with inlined layout ranges
- `ByteRange { offset, len }` for section locations
- `RecordIndexEntry` with fixed 6-field schema (6 × u32 = 24 bytes)
- `BlockBuilder` that accumulates records into block-local buffers
- `BlockDecoder` that holds block-owned bytes and exposes the current record as borrowed slices

### Phase 5: first end-to-end raw round-trip (done)

- file header: 8 bytes (`DRYI` magic + major/minor version as little-endian u16)
- block header: 88 bytes (record count, encoding tags, 5 section ranges)
- all integer fields little-endian throughout the format
- writer emits real file header and serializes blocks with index entries and raw payload sections
- reader parses file header, loads blocks, and exposes the current record via `SeqRecordLike`
- zero-copy primary read path via `next_record()` — no per-record heap allocation
- convenience `into_records()` iterator for users who prefer `for`-loop syntax
- zero-copy reader-to-writer piping: `writer.write_record(&reader)` works because `DryIceReader` implements `SeqRecordLike`

### CI (done)

- GitHub Actions workflow running `just check` on push and PR

## Design commitments that are now proven in code

These are no longer just planning-stage conclusions. They are implemented and tested.

### Record model

- `SeqRecordLike` is the universal record interface, used on both write and read sides
- `SeqRecord` is the owned row-wise output type, used only when ownership is actually needed
- `DryIceReader` itself implements `SeqRecordLike` for the current record — this is the zero-copy path
- one trait, one owned type, no separate borrowed record struct

### Reader access patterns

The reader provides two access patterns:

```rust
// zero-copy primary path
while reader.next_record()? {
    let seq = reader.sequence();
    writer.write_record(&reader)?;
}

// convenience iterator (allocates per record)
for record in reader.into_records() {
    let record = record?;
}
```

### Writer builder

The writer uses `bon`'s function-level builder to ensure the `BlockBuilder` is constructed with the user's actual encoding choices:

```rust
let writer = DryIceWriter::builder()
    .inner(file)
    .sequence_encoding(SequenceEncoding::TwoBitExact)
    .quality_encoding(QualityEncoding::Binned)
    .target_block_records(4096)
    .build();
```

### Binary format

```text
file header (8 bytes)
[4 bytes] magic: DRYI
[2 bytes] version_major: u16 le
[2 bytes] version_minor: u16 le

block header (88 bytes)
[4 bytes]  record_count        u32 le
[1 byte]   sequence_encoding   u8
[1 byte]   quality_encoding    u8
[1 byte]   name_encoding       u8
[1 byte]   sort_key_kind       u8
[16 bytes] index range         offset u64 le + len u64 le
[16 bytes] names range         offset u64 le + len u64 le
[16 bytes] sequences range     offset u64 le + len u64 le
[16 bytes] qualities range     offset u64 le + len u64 le
[16 bytes] sort_keys range     offset u64 le + len u64 le

record index entry (24 bytes)
[4 bytes] name_offset     u32 le
[4 bytes] name_len        u32 le
[4 bytes] sequence_offset u32 le
[4 bytes] sequence_len    u32 le
[4 bytes] quality_offset  u32 le
[4 bytes] quality_len     u32 le
```

### Test coverage

The test suite includes:

- format-level unit tests for file header and block header round-trips
- integration tests covering both zero-copy and iterator access patterns
- single-record, multi-record, multi-block, empty-file, empty-name, long-sequence, and block-boundary-exact cases
- error-path tests for mismatched lengths, bad magic, truncated headers, and unsupported config
- property-based fuzz testing with proptest for arbitrary record round-trip fidelity
- zero-copy reader-to-writer piping test

## What still remains open

These questions are still active, but they are now localized rather than pervasive.

- exact `TwoBitExact` sequence encoding implementation
- exact `Binned` quality encoding implementation
- exact sort-key accelerator implementation and API
- whether `CorruptBlockLayout { message: String }` error variants should use `&'static str` instead
- whether repeated `u32::try_from` patterns in the builder should be factored into helpers
- whether the `BlockBuilder::new` 5-argument constructor should take a config struct instead
- exact `SeqRecord` convenience surface beyond the current constructor/accessor API
- reader-options surface, if any beyond `DryIceReader::new(...)`

## Next implementation priorities

### Phase 6: code quality pass

Before adding new features, address the issues identified during review:

- factor repeated `u32::try_from(...).map_err(...)` patterns into a helper
- consider replacing `String` payloads in error variants with `&'static str` where the messages are compile-time-known
- consider reducing `BlockBuilder::new` argument count
- remove any remaining `#[allow(dead_code)]` annotations that are no longer justified
- verify all `as usize` / `as u64` casts are safe or replace with `try_from`

### Phase 7: `TwoBitExact` sequence encoding

This is the first non-trivial codec and the most important compact encoding for the format.

Conceptual layout:

```text
2-bit canonical stream
+
ambiguity mask
+
ambiguity byte stream
```

This will require:

- encode path in `BlockBuilder`
- decode path in `BlockDecoder`
- round-trip tests with sequences containing IUPAC ambiguity codes
- property-based tests for encoding fidelity

### Phase 8: `Binned` quality encoding

The most promising early lossy transform. Should be cheap enough that it does not undermine throughput.

### Phase 9: concrete sort-key accelerator support

- implement `SortKeyKind::U64Minimizer` and `U128Minimizer` storage
- wire through writer config, block builder, and block decoder
- add sort-key round-trip tests

### Phase 10: benchmarking

Before claiming "high-throughput," we need measurements.

- establish a benchmark harness (likely `criterion`)
- measure write throughput for raw and compact encodings
- measure read throughput for zero-copy and iterator paths
- measure round-trip throughput end-to-end
- compare against raw FASTQ read/write as a baseline

## What should not happen next

The following would be premature right now:

- designing a public plugin system for accelerators
- coupling the core crate to `noodles` or any other parser ecosystem
- designing the Python/Node wrappers before the core encodings are stable
- introducing a macros crate without a concrete need
- building a CLI just because a workspace can contain one
- over-optimizing index schema variations before there is data showing they matter

## Current readiness assessment

The project is past the scaffolding stage. The core architecture is implemented and tested with a working raw-mode round-trip. The zero-copy reader design is in place and proven.

Roughly:

- project identity and scope: strong
- public API philosophy: strong and implemented
- core Rust abstraction model: implemented and tested
- binary format: defined and serializing
- raw-mode round-trip: working
- compact encodings: not yet implemented
- accelerator support: not yet implemented
- benchmarking: not yet started

## Immediate next step

The best next move is the code quality pass (phase 6), followed by `TwoBitExact` encoding (phase 7). The code quality pass is small but will make the codebase cleaner before we add encoding complexity.

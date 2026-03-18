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
- single `dryice` library crate with `thiserror` and `bitnuc` as dependencies, `proptest` as a dev dependency
- `.cargo/config.toml` with `target-cpu=native` for SIMD acceleration
- `justfile` with the standard check/fmt/lint/test/doc workflow
- deny-by-default `.gitignore`
- `AGENTS.md` philosophical primer
- CI via GitHub Actions running `just check`

### Phase 2: public surface skeleton (done)

- `SeqRecordLike` trait with `name()`, `sequence()`, `quality()`, `len()`, `is_empty()` default methods
- `SeqRecordExt` blanket extension trait providing `to_seq_record()`
- `SeqRecord` owned row-wise record type with private fields, invariant-preserving constructors, and accessors
- `DryIceError` with categorized error variants
- public config enums: `NameEncoding`, `BlockSizePolicy`
- `SequenceCodec` trait and built-in implementations: `RawAsciiCodec`, `TwoBitExactCodec`
- `QualityCodec` trait and built-in implementations: `RawQualityCodec`, `BinnedQualityCodec`, `OmittedQualityCodec`

### Phase 3: config/builder layer (done)

- `DryIceWriterOptions` with `name_encoding` and `BlockLayoutOptions` (sequence/quality codecs are type parameters, not config fields)
- fully hand-written typestate builder on `DryIceWriter` with transitions for codec, quality, and key type parameters
- `Default` impls on `NameEncoding` for clean config defaults
- `from_options()` escape hatch that rejects unsupported `TargetBytes` policy
- builder supports typestate transitions: `.sequence_codec::<S>()`, `.two_bit_exact()`, `.quality_codec::<Q>()`, `.binned_quality()`, `.omit_quality()`, `.record_key::<K>()`, `.bytes8_key()`, `.bytes16_key()`

### Phase 4: block schema and internal machinery (done)

- private `BlockHeader` with inlined layout ranges and 16-byte codec type tags
- `ByteRange { offset, len }` for section locations
- `RecordIndexEntry` with fixed 6-field schema (6 × u32 = 24 bytes)
- `BlockBuilder` that accumulates records into block-local buffers using codec function pointers from `BlockBuilderConfig`
- `BlockDecoder` that holds block-owned bytes and decodes via statically-known codec decode functions passed from the reader

### Phase 5: first end-to-end raw round-trip (done)

- file header: 8 bytes (`DRYI` magic + major/minor version as little-endian u16)
- block header: 136 bytes (record count, 16-byte codec type tags, name encoding, record-key metadata, 5 section ranges)
- all integer fields little-endian throughout the format
- writer emits real file header and serializes blocks with index entries and raw payload sections
- reader parses file header, loads blocks, and exposes the current record via `SeqRecordLike`
- zero-copy primary read path via `next_record()` — no per-record heap allocation
- convenience `into_records()` iterator for users who prefer `for`-loop syntax
- zero-copy reader-to-writer piping: `writer.write_record(&reader)` works because `DryIceReader` implements `SeqRecordLike`

### Phase 6: code quality pass (done)

- factored repeated `u32::try_from` patterns into a `to_u32` helper
- replaced `String` payloads in error variants with `&'static str` throughout
- reduced `BlockBuilder::new` to take `&BlockBuilderConfig` instead of 5 positional arguments
- added `SectionOverflow` error variant
- cleaned up `as` casts with explicit `try_from` where appropriate

### Phase 7: generic record-key API (done)

This replaced the earlier `SortKeyKind` enum approach with a fully generic trait-based system.

- `RecordKey` trait with `const WIDTH: u16`, `const TYPE_TAG: [u8; 16]`, `encode_into()`, `decode_from()`
- `NoRecordKey` default marker for unkeyed operation
- built-in key types: `Bytes8Key` (8 bytes) and `Bytes16Key` (16 bytes)
- `DryIceWriter<W, S, Q, K>` and `DryIceReader<R, S, Q, K>` with defaults `S = RawAsciiCodec`, `Q = RawQualityCodec`, `K = NoRecordKey`
- typestate builder transitions: `.record_key::<K>()`, `.bytes8_key()`, `.bytes16_key()`
- keyed write path: `write_record_with_key(&record, &key)`
- keyed read path: `reader.record_key()` returns typed `K`
- reader constructors: `with_record_key::<K>(inner)`, `with_bytes8_key(inner)`, `with_bytes16_key(inner)`
- block header now stores `record_key_width`, `record_key_tag`, and `record_keys` range
- block builder and decoder support keyed sections with type verification
- new error variants: `RecordKeyTypeMismatch`, `MissingRecordKeySection`, `InvalidRecordKeyEncoding`
- round-trip tests for built-in keys, custom user-defined keys, and convenience helper paths

### Phase 8: trait-based sequence codec with TwoBitExact (done)

- `SequenceCodec` trait with `TYPE_TAG`, `LOSSY`, `encode()`, `decode()`
- `RawAsciiCodec` and `TwoBitExactCodec` as built-in implementations
- `TwoBitExactCodec` uses `bitnuc` for SIMD-accelerated 2-bit packing with sparse ambiguity sideband
- sparse sideband format: `[count: u32] [positions: u32 each] [IUPAC bytes]`
- `DryIceWriter` gains `S` type parameter defaulting to `RawAsciiCodec`
- builder transitions: `.sequence_codec::<S>()`, `.two_bit_exact()`
- codec-level unit tests and full integration round-trip tests

### Phase 9: trait-based quality codec with binned and omitted (done)

- `QualityCodec` trait parallel to `SequenceCodec`
- `RawQualityCodec`, `BinnedQualityCodec`, `OmittedQualityCodec` as built-in implementations
- `BinnedQualityCodec` uses Illumina-style 8-level Phred binning
- `DryIceWriter` gains `Q` type parameter defaulting to `RawQualityCodec`
- builder transitions: `.quality_codec::<Q>()`, `.binned_quality()`, `.omit_quality()`
- quality codec unit tests and integration tests including combined codec configurations

### Phase 10: codec type tags in block headers (done)

- replaced `u8` encoding enum tags with 16-byte `TYPE_TAG` values from codec traits
- dropped `ENCODING_TAG` from both `SequenceCodec` and `QualityCodec` traits
- block header grew from 106 to 136 bytes
- `DryIceReader` gains `S` and `Q` type parameters for static codec verification
- reader verifies codec tags at block-load time with `SequenceCodecMismatch` and `QualityCodecMismatch` errors
- removed enum-based `decode_by_tag` dispatch — decoding is fully static
- `SequenceEncoding` and `QualityEncoding` enums removed from public API
- `EncodingOptions` removed; `DryIceWriterOptions` simplified to `name_encoding` + `layout`
- `bon` removed as a dependency
- reader constructors: `with_two_bit_exact()`, `with_sequence_codec::<S>()`, `with_codecs::<S, Q>()`

### CI (done)

- GitHub Actions workflow running `just check` on push and PR

## Design commitments that are now proven in code

These are no longer just planning-stage conclusions. They are implemented and tested.

### Record model

- `SeqRecordLike` is the universal record interface, used on both write and read sides
- `SeqRecord` is the owned row-wise output type, used only when ownership is actually needed
- `DryIceReader` itself implements `SeqRecordLike` for the current record — this is the zero-copy path
- `RecordKey` trait allows user-defined fixed-width accelerator keys with typed encode/decode
- `SequenceCodec` and `QualityCodec` traits allow user-defined encodings with typed encode/decode
- `DryIceWriter<W, S, Q, K>` and `DryIceReader<R, S, Q, K>` are generic over codec and key types, all with sensible defaults
- codec type tags stored directly in block headers; reader verifies at block-load time
- one record trait, one owned record type, two codec traits, one key trait, no separate borrowed record struct

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

The writer uses a hand-written typestate builder with transitions for codec, quality, and key type parameters:

```rust
let writer = DryIceWriter::builder()
    .inner(file)
    .two_bit_exact()
    .binned_quality()
    .target_block_records(4096)
    .build();

let keyed_writer = DryIceWriter::builder()
    .inner(file)
    .two_bit_exact()
    .binned_quality()
    .record_key::<MyKey>()
    .build();

let custom_writer = DryIceWriter::builder()
    .inner(file)
    .sequence_codec::<MyCodec>()
    .quality_codec::<MyQualCodec>()
    .build();
```

### Binary format

```text
file header (8 bytes)
[4 bytes] magic: DRYI
[2 bytes] version_major: u16 le
[2 bytes] version_minor: u16 le

block header (136 bytes)
[4 bytes]  record_count          u32 le
[16 bytes] sequence_codec_tag    [u8; 16]
[16 bytes] quality_codec_tag     [u8; 16]
[1 byte]   name_encoding         u8
[1 byte]   has_record_key        u8
[2 bytes]  record_key_width      u16 le
[16 bytes] record_key_tag        [u8; 16]
[16 bytes] index range           offset u64 le + len u64 le
[16 bytes] names range           offset u64 le + len u64 le
[16 bytes] sequences range       offset u64 le + len u64 le
[16 bytes] qualities range       offset u64 le + len u64 le
[16 bytes] record_keys range     offset u64 le + len u64 le

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
- keyed round-trip tests for built-in 8-byte and 16-byte keys, custom user-defined keys, and convenience helper paths
- TwoBitExact codec unit tests (canonical, ambiguous, all-N, single base, non-multiple-of-32, empty, lowercase normalization)
- TwoBitExact integration round-trip tests (canonical only, ambiguous, multi-block, long sequence with sparse ambiguity)
- quality codec unit tests (raw round-trip, binned idempotency, binned length preservation, binned Phred33 validity, omitted produces empty)
- binned quality integration tests (round-trip, lossiness verification, combined TwoBitExact + binned)

## What still remains open

These questions are still active, but they are now localized rather than pervasive.

- exact `SeqRecord` convenience surface beyond the current constructor/accessor API
- reader-options surface, if any beyond `DryIceReader::new(...)`
- `TwoBitLossyN` sequence codec (defined as enum variant but no codec implementation)
- name encoding as a trait-based system (currently still enum-based, unlike sequence and quality)
- whether the `SequenceEncoding` and `QualityEncoding` internal enums should be removed entirely
- crate-level doc examples in `lib.rs` may reference old builder API and need updating

## Next implementation priorities

### Phase 11: benchmarking

Before claiming "high-throughput," we need measurements.

- establish a benchmark harness (likely `criterion`)
- measure write throughput for raw and compact encodings
- measure read throughput for zero-copy and iterator paths
- measure round-trip throughput end-to-end
- compare against raw FASTQ read/write as a baseline

## What should not happen next

The following would be premature right now:

- coupling the core crate to `noodles` or any other parser ecosystem
- designing the Python/Node wrappers before the core encodings are stable
- introducing a macros crate without a concrete need
- building a CLI just because a workspace can contain one
- over-optimizing index schema variations before there is data showing they matter

## Current readiness assessment

The project has a working, tested implementation with trait-based codec and key APIs, compact sequence encoding, quality binning, and a zero-copy reader. The core architecture is proven through extensive round-trip testing including property-based fuzzing.

Roughly:

- project identity and scope: strong
- public API philosophy: strong and implemented with trait-based extensibility
- core Rust abstraction model: implemented and tested
- binary format: defined and serializing with 16-byte codec type tags
- raw-mode round-trip: working
- compact sequence encoding (TwoBitExact): working with SIMD via bitnuc
- quality binning: working
- generic record-key API: implemented and tested
- trait-based codec extensibility: implemented for both sequence and quality
- benchmarking: not yet started

## Immediate next step

The best next move is benchmarking (phase 11). The core codec and key APIs are complete and tested. Before claiming "high-throughput," we need measurements to validate the design choices and identify optimization opportunities.

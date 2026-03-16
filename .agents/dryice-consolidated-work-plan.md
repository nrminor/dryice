# `dryice` consolidated work plan

## Purpose of this document

This document consolidates the current stable conclusions from the earlier planning artifacts into one implementation-oriented plan.

It is not meant to replace the earlier documents. It is meant to answer a narrower question:

```text
Given what we now know,
what should we build first,
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

## Design commitments that now feel stable

The following points are stable enough to plan implementation around.

### Product / format level

- The file format is block-oriented.
- Records are read-like and row-oriented at the user boundary.
- Names, sequences, and qualities are stored in separate payload streams.
- Each block has a fixed per-record index section.
- Optional accelerator sections are part of the design, but the public API should begin with concrete built-ins rather than generic user-defined extensions.
- Sequence encodings should include at least `RawAscii`, `TwoBitExact`, and likely a lossy `TwoBitLossyN` mode later.
- Quality encodings should begin simply, with `Raw` and `Binned` as the first meaningful choices.

### Rust API level

- The write-side public boundary should be trait-based via `SeqRecordLike`.
- The read-side public boundary should be iterator-based and yield a crate-provided row-wise `SeqRecord` type.
- Per-record dynamic dispatch is unacceptable in hot paths.
- The core crate should remain parser-agnostic.
- The public API should expose project-owned types, not types from external bioinformatics libraries.
- The internal ownership center is block assembly / decode state, not a reusable internal per-record hierarchy.

### Repository / project structure level

- The repo should be reshaped into one real core crate named `dryice` inside a workspace root.
- The old `libdryice`, `dryice` binary stub, and `dryice-macros` structure should be retired.
- The workspace root should be ready for future bindings crates.
- The workflow should be driven by `just`, workspace linting, and explicit tooling.

## What still remains open

These questions are still active, but they are now localized rather than pervasive.

- the exact convenience surface of `SeqRecord`
- the exact field set in private `BlockHeader`
- the exact binary expression of block header and index structures
- the exact builder implementation details and where typestate is justified
- the first reader-options surface, if any beyond `DryIceReader::new(...)`
- the exact first built-in sort-key/accelerator choices
- exact dependency choices beyond a few obvious ones like `thiserror`

These are refinement questions, not existential questions.

## Core API plan

### Write-side boundary

The core write-side user model should be:

```rust
pub trait SeqRecordLike {
    fn name(&self) -> &[u8];
    fn sequence(&self) -> &[u8];
    fn quality(&self) -> &[u8];

    fn len(&self) -> usize {
        self.sequence().len()
    }

    fn is_empty(&self) -> bool {
        self.sequence().is_empty()
    }
}
```

And the writer should work conceptually like:

```rust
pub struct DryIceWriter<W> { ... }

impl<W: std::io::Write> DryIceWriter<W> {
    pub fn builder(inner: W) -> DryIceWriterBuilder<W>;
    pub fn write_record<R: SeqRecordLike>(&mut self, record: &R) -> Result<(), DryIceError>;
    pub fn finish(self) -> Result<W, DryIceError>;
}
```

### Read-side boundary

The read-side user model should be row-wise and iterator-shaped.

```rust
pub struct SeqRecord { ... }

pub struct DryIceReader<R> { ... }

impl<R: std::io::Read> DryIceReader<R> {
    pub fn new(inner: R) -> Result<Self, DryIceError>;
    pub fn records(self) -> DryIceRecords<R>;
}
```

Target user experience:

```rust
let reader = DryIceReader::new(file)?;
for record in reader.records() {
    let record = record?;
    // use SeqRecord
}
```

### `SeqRecord` direction

`SeqRecord` should be:

- row-wise
- owned
- more encapsulated than a bag of public fields
- constructed through invariant-preserving APIs

Current sketch:

```rust
pub struct SeqRecord {
    name: Vec<u8>,
    sequence: Vec<u8>,
    quality: Vec<u8>,
}
```

Likely methods:

- `new(...) -> Result<Self, DryIceError>`
- `from_slices(...) -> Result<Self, DryIceError>`
- `name() -> &[u8]`
- `sequence() -> &[u8]`
- `quality() -> &[u8]`

And `SeqRecord` should likely implement `SeqRecordLike`.

## Internal format / block plan

### Conceptual block picture

The architecture of the block layout is mostly settled.

```text
+---------------------------------------------------------------+
| block header                                                  |
|---------------------------------------------------------------|
| record_count                                                  |
| sequence_encoding                                             |
| quality_encoding                                              |
| name_encoding                                                 |
| sort_key_kind?                                                |
| checksum_kind?                                                |
|                                                               |
| index_range                                                   |
| names_range?                                                  |
| sequences_range                                               |
| qualities_range?                                              |
| sort_keys_range?                                              |
+---------------------------------------------------------------+
| index section                                                 |
|---------------------------------------------------------------|
| entry 0                                                       |
| entry 1                                                       |
| ...                                                           |
+---------------------------------------------------------------+
| names bytes?                                                  |
+---------------------------------------------------------------+
| sequence bytes                                                |
+---------------------------------------------------------------+
| quality bytes?                                                |
+---------------------------------------------------------------+
| sort-key bytes?                                               |
+---------------------------------------------------------------+
```

The main remaining work here is exact Rust and binary schema expression, not rethinking the basic layout.

### Private block schema direction

Current likely private/internal shape:

```rust
struct BlockHeader {
    record_count: u32,
    sequence_encoding: SequenceEncoding,
    quality_encoding: QualityEncoding,
    name_encoding: NameEncoding,
    sort_key_kind: Option<SortKeyKind>,
    checksum_kind: Option<ChecksumKind>,

    index: ByteRange,
    names: Option<ByteRange>,
    sequences: ByteRange,
    qualities: Option<ByteRange>,
    sort_keys: Option<ByteRange>,
}

struct ByteRange {
    offset: u64,
    len: u64,
}

struct RecordIndexEntry {
    name_offset: u32,
    name_len: u32,
    sequence_offset: u32,
    sequence_len: u32,
    quality_offset: u32,
    quality_len: u32,
}
```

Important current decisions:

- `BlockHeader` should own both semantic metadata and layout metadata
- the layout information should be inlined there rather than split into a separate `BlockLayout` type for now
- `ByteRange { offset, len }` is preferred over `Range<u64>` for binary-layout clarity
- the index should remain fixed and explicit initially, even if future optimization opportunities exist
- omitted sections are communicated by the header, and corresponding index fields are ignored when the section is absent for a block

### Internal ownership model

The implementation should be built around block-owned state.

Write path:

```text
T: SeqRecordLike
    -> borrow field slices
    -> validate / derive optional values
    -> append into block-owned buffers
    -> flush encoded block
```

Read path:

```text
dryice bytes
    -> read block
    -> parse header and index
    -> extract one record from payload sections
    -> materialize SeqRecord
```

The likely internal actors are:

- `BlockBuilder`
- encoding-specific block-local buffer state
- `BlockDecoder`
- parsed `BlockHeader`
- parsed `RecordIndexEntry` collection

The current plan does **not** require inventing a reusable internal per-record hierarchy before it is justified.

## Codec plan

The public codec/configuration surface should be enum-based, not trait-based.

Likely public enums:

```rust
pub enum SequenceEncoding {
    RawAscii,
    TwoBitExact,
    TwoBitLossyN,
}

pub enum QualityEncoding {
    Raw,
    Binned,
    Omitted,
}

pub enum NameEncoding {
    Raw,
    Omitted,
}
```

Internal codec implementation machinery can remain flexible and may use traits internally if they buy real simplification.

## Accelerator plan

This is now one of the more important philosophical decisions in the plan.

Current stance:

- public API starts with concrete built-ins
- internal block model still leaves room for multiple optional accelerator sections
- generic user-defined accelerators are deferred until there is real evidence for what they should mean

In other words:

```text
public API now:
  concrete sort-key-oriented choices

internal design now:
  room for multiple optional accelerator sections

public API later, if justified:
  more general extension surface
```

That is not anti-generic. It is sequencing and semver discipline.

## Configuration and builders plan

### User-facing builder shape

The current preferred user-facing configuration shape is:

- flat builder surface
- internally grouped config structs
- sensible defaults
- concrete built-in choices first

Target writer usage:

```rust
let writer = DryIceWriter::builder(file).build()?;

let writer = DryIceWriter::builder(file)
    .sequence_encoding(SequenceEncoding::TwoBitExact)
    .quality_encoding(QualityEncoding::Binned)
    .name_encoding(NameEncoding::Raw)
    .sort_key(SortKeyKind::U128Minimizer)
    .target_block_records(8192)
    .build()?;
```

### Underlying grouped config shape

Likely underlying shape:

```rust
pub struct DryIceWriterOptions {
    pub encoding: EncodingOptions,
    pub layout: BlockLayoutOptions,
    pub sort_key: Option<SortKeyKind>,
}

pub struct EncodingOptions {
    pub sequence: SequenceEncoding,
    pub quality: QualityEncoding,
    pub names: NameEncoding,
}

pub struct BlockLayoutOptions {
    pub block_size: BlockSizePolicy,
    pub checksum: Option<ChecksumKind>,
}

pub enum BlockSizePolicy {
    TargetRecords(usize),
    TargetBytes(usize),
}
```

### Builder implementation strategy

Current stance:

- `bon` is a strong candidate for public-facing builders with multiple optional fields
- typestate builders are welcome when they genuinely prevent invalid states and improve UX
- typestate should not be introduced just to show off the type system
- plain builders remain appropriate for smaller or purely internal construction paths

Current likely defaults:

- `sequence_encoding = RawAscii`
- `quality_encoding = Raw`
- `name_encoding = Raw`
- `sort_key = None`
- `block_size = TargetRecords(<sensible default>)`
- `checksum = None`

### Reader configuration stance

Reader configuration should start much thinner than writer configuration.

Current bias:

- default path should be `DryIceReader::new(inner)`
- optional reader builder/config should only appear if operational tuning knobs become justified
- reader configuration should mostly be operational, not semantic, because the file largely describes how it must be interpreted

## Error model plan

The current direction is a single top-level `DryIceError` using ordinary `thiserror` patterns.

The important family shape is:

- transport/runtime errors
- configuration errors
- input-record validity errors
- file identity/version errors
- structural corruption errors
- unsupported feature/encoding errors
- integrity/decode failures

Current principles:

- one top-level `DryIceError` is the right starting point
- configuration errors may grow typed sub-errors later
- corruption and unsupported-feature cases should remain distinct
- some string payloads are acceptable early, but the taxonomy should still be meaningful

## Repository restructuring plan

The next repository reshape should aim for:

```text
workspace root
+- Cargo.toml
+- Cargo.lock
+- justfile
+- .gitignore
+- README.md
+- rustfmt.toml
+- AGENTS.md
+- .agents/
+- dryice/
   +- Cargo.toml
   +- src/
```

And explicitly remove or retire:

- the current `dryice` binary-stub crate shape
- `libdryice`
- `dryice-macros`

The first module tree should stay modest and concept-oriented, something like:

```text
src/
+- lib.rs
+- error.rs
+- record.rs
+- format/
+- block/
+- codec/
+- io/
+- accelerator/
```

## Immediate implementation priorities

Once the repo is restructured, the first implementation work should probably proceed in this order.

### Phase 1: repo and crate skeleton

- rewrite the workspace root manifest
- create the single real `dryice` crate
- add `AGENTS.md`, `justfile`, and initial root `.gitignore`
- create the initial module tree
- add crate-level docs and minimal public re-exports

### Phase 2: public surface skeleton

- define `DryIceError`
- define `SeqRecordLike`
- define initial `SeqRecord`
- define the public encoding/config enums
- sketch `DryIceWriter<W>` and `DryIceReader<R>` APIs without full implementation

### Phase 3: config/builder layer

- define `DryIceWriterOptions` and grouped config structs
- add builder story, likely with `bon`
- settle whether any typestate builder is actually justified

### Phase 4: block schema and internal machinery skeleton

- define private `BlockHeader`
- define `ByteRange`
- define `RecordIndexEntry`
- define `BlockBuilder` and `BlockDecoder` skeletons
- wire writer/reader internals around block-centered state

### Phase 5: first end-to-end path

- support a simple writer path using `SequenceEncoding::RawAscii`, `QualityEncoding::Raw`, and `NameEncoding::Raw`
- support matching read path back into `SeqRecord`
- get a minimal format round-trip working before more sophisticated encodings

### Phase 6: richer encodings and accelerators

- add `TwoBitExact`
- add `Binned` qualities
- add concrete built-in sort-key support if justified by the first integration work

## What should not happen next

The following would be premature right now:

- designing a public plugin system for accelerators
- coupling the core crate to `noodles` or any other parser ecosystem
- designing the Python/Node wrappers before the Rust API skeleton exists
- introducing a macros crate without a concrete need
- building a CLI just because a workspace can contain one
- over-optimizing index schema variations before there is data showing they matter

## Current readiness assessment

At this point, the project is not ready to hand entirely to implementation agents with no supervision, but it is ready for disciplined scaffolding and early core-type work.

Roughly:

- project identity and scope: strong
- public API philosophy: strong
- core Rust abstraction model: strong enough to scaffold
- internal block/header/index picture: strong enough to scaffold
- implementation plan: now actionable in early phases

## Immediate next step

The best next move after this plan is to perform the repository restructuring.

That means:

- collapse to one real core crate
- install the root workflow/tooling files
- create the initial module skeleton
- then start implementing the public surface in the order described above

That should let the project move from design-heavy planning into design-guided construction without losing the care and discipline that got it here.

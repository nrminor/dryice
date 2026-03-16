# `dryice` Rust design notebook

## How to use this document

This is a living design notebook.

It is not meant to read like polished project documentation. Its job is to capture the active design state of the `dryice` Rust library while we think through the core abstractions before implementing them.

The two earlier documents establish the current foundation:

- `.agents/dryice-design-first-pass.md`
- `.agents/dryice-rust-architecture-plan.md`

This notebook sits on top of those and asks a narrower question:

```text
Given the current plan for dryice,
what is the right Rust expression of the core ideas?
```

We should expect this document to change repeatedly.

## Current bedrock assumptions

These are treated as stable enough to design around for now.

1. `dryice` is a high-throughput transient container for read-like genomic records.
2. The format is block-oriented.
3. The conceptual record shape includes name, sequence, quality, and small flags/metadata.
4. The format should support optional accelerator arrays, especially sort keys.
5. The core crate should remain parser-agnostic.
6. The core public API should expose project-owned types rather than types from external bioinformatics libraries.
7. Python and Node wrappers are expected later, so wrapper-friendliness is a present design constraint.
8. The project will start as a single core Rust library crate inside a workspace.
9. The write-side public boundary is likely trait-based, centered on a sequencing-record interface like `SeqRecordLike`.
10. The read-side public boundary is likely iterator-based, centered on a crate-provided row-wise output type like `SeqRecord`.
11. Per-record dynamic dispatch in hot paths is unacceptable.
12. The internal ownership center is more likely block assembly / decode state than a reusable internal per-record record hierarchy.
13. The architectural block layout is mostly settled; remaining work is about Rust/schema expression of that layout.

These are not eternal truths, but they are solid enough that design work should not keep re-litigating them unless something significant changes.

## The main Rust design problem

The problem is no longer mainly about the file format sketch.

It is now about deciding which conceptual pieces of the format should become first-class Rust primitives and which should remain implementation details.

At the highest level, the conceptual stack currently looks like this:

```text
record -> block -> section -> codec -> reader/writer
```

Each layer now needs to be challenged.

For each one, we need to ask:

- is this a real library primitive or just a useful mental model?
- should it be public or internal?
- should it be represented by structs, enums, traits, or some combination?
- does it need both owned and borrowed forms?
- does it need static dispatch, dynamic dispatch, or neither?

## First-pass public primitive candidates

These are the concepts that currently look most likely to deserve public representation.

### 1. Sequencing-record interface

This now looks like the most important public write-side primitive.

Current direction:

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

Why this looks strong:

- users can keep their own record types
- `dryice` remains parser-agnostic
- write-side ingestion can use static dispatch via generics
- no public input record struct is required as the center of the API
- implementors get a small amount of useful record-shaped convenience for free

This trait should stay intentionally narrow for now.

### Possible extension trait

If later we want to provide genuinely richer conveniences without bloating the core interoperability boundary, a blanket extension trait is plausible.

Current likely example:

```rust
pub trait SeqRecordExt: SeqRecordLike {
    fn to_seq_record(&self) -> Result<SeqRecord, DryIceError> {
        SeqRecord::from_slices(self.name(), self.sequence(), self.quality())
    }
}

impl<T: SeqRecordLike + ?Sized> SeqRecordExt for T {}
```

Current bias:

- `len()` and `is_empty()` belong on `SeqRecordLike` itself
- `SeqRecordExt` is only justified if it offers real ergonomic upgrades like conversion into `SeqRecord`

### 2. Row-wise output record

The read side probably still wants a crate-provided row-wise output type for ergonomic iteration.

Current direction:

- use a public `SeqRecord` output type
- do not treat that type as the canonical internal format primitive
- treat it as the row-wise decoded/output view users most often consume

Current additional bias:

- `SeqRecord` should be more encapsulated than a bag of public fields
- construction should preserve invariants up front rather than relying on later validation
- `SeqRecord` should likely implement `SeqRecordLike`

This is a deliberate asymmetry with the write side.

### 3. Block

This also feels likely to be real.

The format is block-oriented, and some workflows may want to reason about blocks directly. But there is still an open question about how much of the block model should be surfaced publicly versus hidden behind readers and writers.

Likely public roles:

- block metadata/header view
- maybe block reader/writer interfaces

Potential risk:

- exposing too much block machinery too early could freeze internal layout decisions prematurely

### 4. Codec configuration

Users will likely need to choose sequence and quality encoding strategies.

That means some public configuration vocabulary seems necessary, even if the actual codec implementations remain internal.

Likely public roles:

- sequence encoding choice enum
- quality encoding choice enum
- maybe a top-level writer config struct

### 5. Reader/writer API

This is almost certainly public.

The open question is not whether it exists, but what style it should take.

### 6. Accelerator selection

This probably belongs in the public API eventually, but the first version may want a very narrow expression of it rather than a highly generic framework.

## First-pass internal primitive candidates

These are concepts that may be real internally without necessarily becoming central public APIs.

### Section

The file-format sketch naturally talks about sections.

But it is still not obvious that users should manipulate sections directly. They may remain a format/internal concern expressed through:

- file headers
- block headers
- encoded block views

instead of through a public `Section` abstraction.

### Index entries

Blocks need fixed-width per-record indexing internally. But whether users should manipulate raw index entries directly is much less clear.

My current bias is:

- index entries are probably internal or semi-internal
- block traversal APIs can expose record views without forcing users to think in offsets
- the first index shape should be fixed and explicit, even if later optimization opportunities exist

### Codec implementations

The codec identities should likely be public.

The codec trait or internal encode/decode machinery may not need to be.

### Adapter mechanisms

The core crate will need some strategy for taking in data from different parser ecosystems, but the exact mechanism may remain mostly outside the public story of the core crate.

## Record model

The record abstraction is still important, but the discussion clarified that the write-side and read-side record stories should probably not be symmetric.

## Current direction

Current likely split:

- write side: trait-based boundary via `SeqRecordLike`
- read side: crate-provided row-wise output type `SeqRecord`
- internals: still private and not yet committed to a reusable per-record type hierarchy

This is a better fit than forcing a single public record abstraction to serve all roles.

## Current note on public record types

We should be careful not to over-export implementation choices.

The current likely outcome is:

- `SeqRecordLike` is the public input interface
- `SeqRecord` is the public output value type
- internal record storage/transformation types, if any, stay private for now

This keeps semver surface smaller while still allowing a pleasant read API.

### Current `SeqRecord` sketch

```rust
pub struct SeqRecord {
    name: Vec<u8>,
    sequence: Vec<u8>,
    quality: Vec<u8>,
}

impl SeqRecord {
    pub fn new(
        name: Vec<u8>,
        sequence: Vec<u8>,
        quality: Vec<u8>,
    ) -> Result<Self, DryIceError> {
        if sequence.len() != quality.len() {
            return Err(DryIceError::MismatchedSequenceAndQualityLengths {
                sequence_len: sequence.len(),
                quality_len: quality.len(),
            });
        }

        Ok(Self {
            name,
            sequence,
            quality,
        })
    }

    pub fn from_slices(
        name: &[u8],
        sequence: &[u8],
        quality: &[u8],
    ) -> Result<Self, DryIceError> {
        Self::new(name.to_vec(), sequence.to_vec(), quality.to_vec())
    }

    pub fn name(&self) -> &[u8] {
        &self.name
    }

    pub fn sequence(&self) -> &[u8] {
        &self.sequence
    }

    pub fn quality(&self) -> &[u8] {
        &self.quality
    }
}
```

This sketch is intentionally conservative:

- private fields
- accessor methods
- construction through invariant-preserving constructors
- richer convenience APIs can be added later without making the type sloppy

### Current `SeqRecordLike` relationship

Current likely direction:

```rust
impl SeqRecordLike for SeqRecord {
    fn name(&self) -> &[u8] {
        self.name()
    }

    fn sequence(&self) -> &[u8] {
        self.sequence()
    }

    fn quality(&self) -> &[u8] {
        self.quality()
    }
}
```

This keeps the trait as the interoperability/input boundary while allowing the crate's own row-wise output type to participate in the same ecosystem naturally.

## Questions on the record model

1. What exact convenience surface should `SeqRecord` expose beyond the minimal constructor/accessor API?
2. Should `SeqRecordLike` stay limited to `name/sequence/quality` for now?
3. Do we want any public flags/metadata on `SeqRecord` immediately, or should that wait?
4. How much validation should happen at write-time ingestion versus deeper in block assembly?

## Block model

The format is block-oriented, but that does not automatically tell us the right public Rust shape.

The recent diagram work suggests something more specific:

- blocks are central internally
- block-owned buffers and decode state are likely the real internal ownership center
- this is more important than inventing a reusable private per-record hierarchy too early

### Important clarification

At this point, the high-level block layout is already fairly well specified conceptually.

What remains open is not "what is a block?" in the abstract. What remains open is:

- the Rust-side schema expression of that layout
- the exact on-disk expression of section presence and block metadata
- the exact relationship between block headers, section ranges, and the index in Rust types

So future design work in this area should focus on schema expression and representation, not on reopening the whole architectural idea of block-oriented storage from scratch.

## Candidate public block concepts

Possible public block-facing types:

- `BlockHeader`
- `BlockInfo`
- `BlockView<'a>`
- `OwnedBlock`

But this is a place to be cautious.

We probably do not want the first user experience to be “please construct your own blocks.” More likely, users should write records and let the writer assemble blocks.

So the current bias is:

- block metadata may be public
- full block construction may remain internal initially

### Current internal block schema direction

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

Current reasoning:

- `BlockHeader` should own both semantic metadata and layout metadata
- the layout information does not need to be factored into a separate `BlockLayout` type unless later complexity justifies it
- `ByteRange { offset, len }` reads more naturally for binary layout work than `Range<u64>` or a forest of tiny wrapper structs
- the index should start as a fixed explicit schema rather than varying with section presence/absence
- if names or qualities are omitted for a whole block, the header communicates that, and the corresponding index fields are simply ignored for that block

## Current note on optional block contents

Block contents should not be modeled as if every section is always present.

The current conceptual block shape is:

```text
block
+- required header
+- required index
+- optional names section
+- required sequence section
+- optional quality section
+- optional accelerator section(s)
+- possibly additional optional metadata/stat sections later
```

This should influence both the internal layout model and the configuration model.

## Candidate sketch: block metadata only

```rust
pub struct BlockInfo {
    pub record_count: u32,
    pub sequence_encoding: SequenceEncoding,
    pub quality_encoding: QualityEncoding,
    pub accelerator_kinds: Vec<AcceleratorKind>,
}
```

This kind of type could be useful on the read side without overcommitting the internal representation.

## Candidate sketch: internal block view

```rust
pub(crate) struct EncodedBlock<'a> {
    header: &'a BlockHeader,
    index_bytes: &'a [u8],
    name_bytes: &'a [u8],
    sequence_bytes: &'a [u8],
    quality_bytes: &'a [u8],
    accelerator_bytes: &'a [u8],
}
```

This is likely closer to what the internals want than what the public API wants.

### Current block picture

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

Important note:

- the header, index, and payload relationship is conceptually settled enough to design around
- the remaining work is in exact binary and Rust expression details, not in rethinking the existence of these pieces

## Section model

Sections are real in the format sketch, but still questionable as public library primitives.

Current bias:

- section vocabulary should probably exist in the internal format model
- users should more often reason in terms of records, blocks, and encodings than raw sections

So a likely shape is:

- public: configuration and metadata
- internal: concrete section layout and offset machinery

## Codec model

This is one of the most delicate areas.

We know `dryice` needs pluggable encodings conceptually. But “pluggable encodings” can be expressed in Rust in several very different ways.

## Options on the table

### Option A: public enums, internal match dispatch

```rust
pub enum SequenceEncoding {
    RawAscii,
    TwoBitExact,
    TwoBitLossyN,
}
```

and then internally:

```rust
match encoding {
    SequenceEncoding::RawAscii => ...,
    SequenceEncoding::TwoBitExact => ...,
    SequenceEncoding::TwoBitLossyN => ...,
}
```

Why this is attractive:

- simple
- explicit
- wrapper-friendly
- no trait-object machinery required

Why it may become limiting:

- internal code paths may become crowded as the number of codecs grows

Current bias:

- this is the best initial choice

### Option B: public codec traits

```rust
pub trait SequenceCodec {
    ...
}
```

Why this is tempting:

- extensibility
- apparent abstraction purity

Why I am skeptical:

- probably too much polymorphism too early
- pushes complexity into the public API
- can make wrappers and configuration stories worse
- risks designing for hypothetical external codec plugins before the core format exists

Current bias:

- avoid public codec traits early

### Option C: internal codec traits only

This may be the sweet spot.

Public API:

- enums and config

Internal implementation:

- traits or helper types if they reduce duplication cleanly

Current bias:

- plausible and likely healthy

## Current codec conclusion

Best current working assumption:

- public codec selection should be enum-based
- internal codec implementation strategy can remain flexible
- do not commit the public API to trait-driven codec polymorphism early

The current likely home for codec-specific ownership/state is inside block assembly and block decode machinery.

## Reader/writer model

The first public operational API probably wants to be record-oriented at the edges and block-oriented internally.

That means users should likely be able to do things conceptually like:

```text
push records into writer
flush/finish writer

open reader
iterate records or record views
inspect metadata if needed
```

without having to assemble blocks themselves.

### Current write-side sketch

```rust
pub struct DryIceWriter<W> {
    inner: W,
    block_builder: BlockBuilder,
    options: DryIceWriterOptions,
}

impl<W: std::io::Write> DryIceWriter<W> {
    pub fn write_record<R: SeqRecordLike>(&mut self, record: &R) -> Result<(), DryIceError> {
        self.block_builder.push_record(record)?;

        if self.block_builder.should_flush() {
            self.flush_block()?;
        }

        Ok(())
    }
}
```

This looks strong because:

- `SeqRecordLike` is enough at the ingestion boundary
- ownership likely moves into block-owned buffers at `push_record`
- no internal per-record type is required yet to justify the sketch

### Current read-side sketch

```rust
pub struct DryIceReader<R> {
    inner: R,
    block_decoder: Option<BlockDecoder>,
}

impl<R: std::io::Read> DryIceReader<R> {
    pub fn records(self) -> DryIceRecords<R> {
        ...
    }
}
```

Current read-side bias:

- iterator-shaped API
- yields `Result<SeqRecord, DryIceError>`
- avoids per-record dynamic dispatch
- keeps the user-side call pattern natural in Rust

### Current simple reader path

The default read path should likely stay extremely small:

```rust
let reader = DryIceReader::new(file)?;
for record in reader.records() {
    let record = record?;
    // use SeqRecord
}
```

The current bias is that `DryIceReader::new(inner)` should be the primary entrypoint, with a builder-based reader configuration story only if operational tuning knobs become justified.

## Adapter boundary

We expect `dryice` to sit in an ecosystem with multiple FASTQ and sequencing parsers.

The project needs a strategy for this, but that strategy should not infect the core crate too early.

## Candidate adapter attitudes

### A. No adapters in the core crate initially

Users convert into `dryice::Record` themselves.

Pros:

- smallest core API
- strongest parser independence
- easiest to reason about

Cons:

- more friction for early adopters

### B. Conversion traits owned by `dryice`

Conceptually:

```rust
pub trait IntoRecord {
    fn into_record(self) -> Result<Record, Error>;
}
```

Why I am cautious:

- this can become an unnecessary generic layer quickly
- orphan rules and external trait impl ergonomics may be awkward

### C. Adapter crates later

Examples:

- `dryice-noodles`
- `dryice-fastq`

This seems architecturally clean, but should not be added until the core model is stable.

## Current adapter conclusion

Best current working assumption:

- core crate remains adapter-free at first
- public record model is designed to make adapters easy later
- revisit adapter crates only after the core API exists

Users of parser ecosystems like `noodles` should remain front of mind, but without allowing those ecosystems to dictate the core API.

## Design ethic for complexity

We should not confuse "simplicity" with "making users do more work."

Current design principle:

```text
be conservative about public commitments
but aggressive about doing hard design work
when it creates real user-facing clarity, safety, or power
```

In practice, that means:

- avoid speculative sophistication that only serves internal elegance
- absolutely use Rust's type system when it can make invalid states harder to express or APIs substantially nicer to use
- do not shy away from internal complexity if it buys durable ergonomic or correctness benefits for users

This should remain a guiding principle for configuration APIs, builders, trait boundaries, and iterator design.

## Wrapper-facing constraints

Python and Node support should remain visible throughout this design process.

That does not mean the Rust API should be dumbed down. It means it should avoid avoidable hostility.

## Good wrapper-facing properties

- public config carried in simple structs/enums
- public record model uses owned bytes and simple flags
- public APIs centered on obvious operations
- errors can be translated to foreign-language exceptions without guesswork

## Risky wrapper-facing properties

- public APIs dominated by borrowed lifetimes
- public traits users must implement
- public generic abstractions that are elegant in Rust but painful across FFI boundaries
- deep dependence on external crate types

## Current wrapper conclusion

The Rust API should be written as if a wrapper author is a first-class user.

That likely means:

- concrete public types
- a restrained public trait surface
- explicit configuration
- no casual leakage of internal lifetime machinery into the top-level API

This now aligns better with the current split:

- trait-based write boundary
- concrete row-wise read output
- private block-centered internals

## Generics vs enums vs traits

This is likely to be a recurring theme.

## Current rules of thumb

1. Use plain structs for the core data model unless there is a strong reason not to.
2. Use enums for public configuration and mode selection.
3. Use traits internally when they simplify implementation meaningfully.
4. Avoid public trait-based extensibility until there is concrete evidence that multiple implementations are worth supporting.
5. Be suspicious of trait objects in the public API unless they buy something major.

This is a bias, not a law, but it is probably the right bias for this project.

## Possible initial public API shape

This is a deliberately small sketch, but the current direction has shifted.

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

pub struct SeqRecord { ... }

pub struct DryIceWriterOptions { ... }

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

pub struct DryIceWriter<W> { ... }
pub struct DryIceReader<R> { ... }

impl<W: std::io::Write> DryIceWriter<W> {
    pub fn builder(inner: W) -> DryIceWriterBuilder<W>;
    pub fn write_record<R: SeqRecordLike>(&mut self, record: &R) -> Result<(), Error>;
    pub fn finish(self) -> Result<W, Error>;
}

impl<R: std::io::Read> DryIceReader<R> {
    pub fn new(inner: R) -> Result<Self, Error>;
    pub fn records(self) -> DryIceRecords<R>;
}
```

This may not be the final shape, but it is a much better current baseline than the earlier sketches.

## Configuration and builders

The current direction for user-facing configuration is:

- explicit config structs
- builder-based construction for larger option sets
- likely use of `bon` for public-facing builders where it improves ergonomics
- typestate builders only where omission of required choices would lead to invalid or misleading states

This likely applies to things like:

- writer options
- reader options
- block/layout options
- encoding option groups
- accelerator options

The current design ethic here is important: builders and type-driven configuration are exactly the kind of place where library authors should be willing to do substantial work if it produces a meaningfully better user experience.

## Current configuration shape

The current direction is:

- flat builder surface for users
- internally grouped configuration structs
- concrete built-in choices first
- room for future growth without immediately exposing nested builder chains

### Current writer options sketch

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

pub enum BlockSizePolicy {
    TargetRecords(usize),
    TargetBytes(usize),
}

pub enum SortKeyKind {
    U64Minimizer,
    U128Minimizer,
}
```

This is not meant as a final field list. It is a structural sketch.

### Current writer builder user experience

Minimal path:

```rust
let writer = DryIceWriter::builder(file).build()?;
```

More explicit path:

```rust
let writer = DryIceWriter::builder(file)
    .sequence_encoding(SequenceEncoding::TwoBitExact)
    .quality_encoding(QualityEncoding::Binned)
    .name_encoding(NameEncoding::Raw)
    .sort_key(SortKeyKind::U128Minimizer)
    .target_block_records(8192)
    .build()?;
```

Current conclusions embedded in this sketch:

- user-facing builder should be flat first
- internal configuration can still be grouped by concern
- accelerator configuration should start with concrete built-ins like `sort_key(...)`
- `target_block_records(...)` is likely a nicer public builder method than forcing users to construct `BlockSizePolicy` directly

### Defaults currently implied by the sketch

The current default mental model is roughly:

- `sequence_encoding = SequenceEncoding::RawAscii`
- `quality_encoding = QualityEncoding::Raw`
- `name_encoding = NameEncoding::Raw`
- `sort_key = None`
- `block_size = BlockSizePolicy::TargetRecords(<sensible default>)`
- `checksum = None`

These defaults are not final, but they are a useful starting point for reasoning about the API.

### `bon` and builders

Current likely use of `bon`:

- top-level public-facing builders for writer/reader/config objects with several optional fields
- explicit defaults and ergonomic setter names
- possibly typestate builders only where there is a truly required choice with no good default

Current likely non-use of `bon`:

- tiny internal structs
- internal state objects that are not part of a user-facing construction story

## Current note on accelerators

The current best compromise is:

- public API starts with concrete built-in accelerator concepts, especially sort-key-oriented ones
- internal block/layout model still leaves room for multiple optional accelerator sections
- user-defined or plugin-like accelerator extension is deferred until there is real evidence for what that should mean

This is not a rejection of generic programming. It is a sequencing choice.

We should remain willing to do substantial type-system and API design work later if it creates a materially better user experience for extensibility, but we should not invent that surface before the first real built-ins are well understood.

## Error model

The current direction is a single top-level `DryIceError` using normal `thiserror` patterns, with clear internal category boundaries.

### First-pass family shape

```rust
#[derive(Debug, Error)]
pub enum DryIceError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid writer configuration: {0}")]
    InvalidWriterConfiguration(&'static str),

    #[error("invalid reader configuration: {0}")]
    InvalidReaderConfiguration(&'static str),

    #[error(
        "sequence and quality lengths differ: sequence={sequence_len}, quality={quality_len}"
    )]
    MismatchedSequenceAndQualityLengths {
        sequence_len: usize,
        quality_len: usize,
    },

    #[error("record is missing required field: {field}")]
    MissingRequiredField {
        field: &'static str,
    },

    #[error("invalid sequence encoding input: {message}")]
    InvalidSequenceInput {
        message: String,
    },

    #[error("invalid quality encoding input: {message}")]
    InvalidQualityInput {
        message: String,
    },

    #[error("unsupported format version: {version}")]
    UnsupportedFormatVersion {
        version: u32,
    },

    #[error("invalid file magic bytes")]
    InvalidMagic,

    #[error("corrupt block header: {message}")]
    CorruptBlockHeader {
        message: String,
    },

    #[error("corrupt block layout: {message}")]
    CorruptBlockLayout {
        message: String,
    },

    #[error("corrupt record index at entry {entry}: {message}")]
    CorruptRecordIndex {
        entry: usize,
        message: String,
    },

    #[error("section `{section}` is missing but required by this block")]
    MissingRequiredSection {
        section: &'static str,
    },

    #[error("section `{section}` is present but not valid for this block")]
    UnexpectedSection {
        section: &'static str,
    },

    #[error("unsupported sequence encoding: {encoding:?}")]
    UnsupportedSequenceEncoding {
        encoding: SequenceEncoding,
    },

    #[error("unsupported quality encoding: {encoding:?}")]
    UnsupportedQualityEncoding {
        encoding: QualityEncoding,
    },

    #[error("unsupported name encoding: {encoding:?}")]
    UnsupportedNameEncoding {
        encoding: NameEncoding,
    },

    #[error("unsupported sort key kind: {kind:?}")]
    UnsupportedSortKeyKind {
        kind: SortKeyKind,
    },

    #[error("block checksum mismatch")]
    ChecksumMismatch,

    #[error("record {record_index} could not be decoded: {message}")]
    RecordDecode {
        record_index: usize,
        message: String,
    },
}
```

### Current conclusions about errors

The important part is the family shape, not the exact variant names.

The categories currently look like:

- transport/runtime errors
- configuration errors
- input-record validity errors
- file identity/version errors
- structural corruption errors
- unsupported feature/encoding errors
- integrity/decode failures

Current biases:

- one top-level `DryIceError` is the right starting point
- configuration errors may deserve more typed sub-errors later
- corruption and unsupported-feature cases should remain distinct
- some string payloads are fine early, but the taxonomy should still be meaningfully structured
- use `thiserror` in ordinary Rust-library style

## Questions for the next round of design

1. What exact convenience surface should `SeqRecord` expose beyond the minimal constructor/accessor API?
2. How should the internal block/layout model represent required vs optional sections?
3. What exact fields should the private `BlockHeader` carry beyond the current sketch?
4. What exact shape should writer/reader configuration take?
5. What should the first `DryIceReader` options/configuration surface look like?
6. Where is `bon` helpful, and where would plain builders or typestate builders be cleaner?
7. Does the first writer need batch-oriented methods in addition to one-record-at-a-time APIs?
8. Does the crate need any public block metadata type immediately, or can block concepts stay behind the reader/writer API at first?

## What this document is not deciding yet

This notebook is not yet trying to settle:

- exact module file names
- exact dependency choices
- exact error enum contents
- exact binary layout structs
- exact buffer management strategy
- benchmark methodology

Those all matter, but they should sit downstream of the primitive and boundary decisions above.

## Current state in one picture

```text
stable enough
-------------
transient container
block-oriented format
read-like records
parser-agnostic core
future wrappers
trait-based write boundary
iterator-based read boundary
block-owned internal ownership center

active design work
------------------
SeqRecord convenience surface
block header details and binary expression
config/builder design
enum vs trait codec strategy internally
reader options shape
wrapper-friendly public surface
```

## Working conclusion

The strongest current direction is:

- trait-based write/input boundary via `SeqRecordLike`
- crate-provided row-wise read/output type `SeqRecord`
- block-oriented internals with block-owned assembly/decode state
- private `BlockHeader` with inlined layout ranges is the current best block schema expression
- fixed explicit `RecordIndexEntry` is the current best starting index shape
- enum-driven codec/config selection in the public surface
- explicit optional block sections in the internal layout model
- public accelerator API should start with concrete built-in concepts (for example sort-key-oriented choices) rather than a generic extension mechanism
- internal block/layout model should still leave room for multiple optional accelerator sections
- flat user-facing builders over internally grouped configuration, with `bon` as a likely tool for larger public option sets
- parser independence preserved by keeping adapters out of the core early
- wrappers treated as future first-class consumers of the Rust API

That is the current design center unless later sketching and discussion reveal a better one.

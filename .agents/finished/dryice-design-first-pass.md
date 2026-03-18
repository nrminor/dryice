# `dryice`: first-pass design

## Problem statement

`dryice` is a high-throughput transient container for read-like genomic records.

Its job is to make it cheap to move large collections of sequencing records onto disk and back again when RAM is bounded, while preserving the structural advantages of genomic data over generic byte blobs. It is optimized for temporary, local, batch-oriented persistence rather than long-term archival, exchange, or ecosystem-wide standardization.

The motivating use case is external sorting of very large sequencing datasets, especially global sequence-based sorting of FASTQ-like reads for downstream compression and locality benefits. But the format should also support nearby workloads that need the same thing: spill to disk now, recover quickly later, and avoid paying unnecessary text parsing or general-purpose serialization overhead in between.

## What `dryice` is for

`dryice` should be a good fit when all of the following are true:

- the data consists of read-like genomic records with fields such as name, sequence, and qualities
- the data is too large, bursty, or inconvenient to keep entirely in memory
- the records need to be reloaded quickly, often in large sequential batches
- the persistence is temporary and under local control
- throughput matters more than interoperability or schema evolution

The flagship case remains external disk-spilling workflows, especially k-way merge sorting, but the design should stay useful for other nearby out-of-core pipelines.

## What `dryice` is not for

`dryice` is not intended to be:

- a general archival genomics file format
- a replacement for FASTQ, BAM/CRAM, or other established persistent formats
- a general-purpose columnar analytics format
- the right format for arbitrary genomic transformations or broad downstream interchange
- a place to hide heavy compression or clever but expensive codecs in the hot path

This is a temporary format first. If a design choice makes the transient workflow worse in order to make the format more universal, that is probably the wrong choice.

## Primary use cases

The design should keep these applications front of mind.

### 1. External sorting of sequencing reads

This is the main anchor use case.

The format should support:

- spilling sorted runs or partitions to disk
- reloading them for k-way merge
- optional storage of precomputed sort keys or related accelerator arrays
- cheap sequential scan with minimal decode work in the comparison path

### 2. Temporary partitioning and bucketing

Many sequence-processing pipelines need to group reads into buckets before later processing. Examples include minimizer-based partitioning, sketch-based bucketing, and other locality-preserving sharding steps.

The format should make it easy to write out batches, reopen them later, and keep the record structure intact without reparsing FASTQ text.

### 3. Spill-backed deduplication and grouping workflows

Some deduplication, UMI grouping, and read-collation jobs have sort-like or shuffle-like intermediate stages. `dryice` should be able to serve as fast backing storage for these phases even when the exact derived key is not the final sorting key.

### 4. Temporary staging between asymmetric pipeline phases

One pipeline stage may produce records faster than the next stage can consume them. `dryice` should be useful as a local, structured spill format for buffering these bursts without paying a large CPU tax.

### 5. Read-pair, barcode, and UMI regrouping

Some workflows need to reorganize read-like records into a different access pattern before the next step. `dryice` should support temporary persistence in those regrouping phases, especially when the pipeline benefits from keeping side information near the records.

## Core design stance

The format should be designed as a block-oriented temporary genomic record container with pluggable encodings.

That means:

- the unit of storage is a block, not an individually serialized opaque record
- record structure remains explicit
- payload streams are separated by field type where that helps throughput and traversal
- optional derived per-record arrays are first-class when they materially help nearby workloads
- compact genomic encodings are supported, but should not burden the fast path when raw storage is better

This is the architectural center of gravity for `dryice`.

## High-level file layout

The current working model is:

```text
+----------------------------------------------------------------------------------+
| file header                                                                      |
|----------------------------------------------------------------------------------|
| magic | version | flags | codec ids | optional metadata                          |
+----------------------------------------------------------------------------------+

+----------------------------------------------------------------------------------+
| block 0                                                                          |
|----------------------------------------------------------------------------------|
| block header                                                                      |
|   record_count                                                                    |
|   seq_encoding                                                                    |
|   qual_encoding                                                                   |
|   name_encoding                                                                   |
|   key_encoding                                                                    |
|   section sizes / offsets                                                         |
|----------------------------------------------------------------------------------|
| optional side arrays                                                              |
|----------------------------------------------------------------------------------|
| record index table                                                                |
|----------------------------------------------------------------------------------|
| name payload stream                                                               |
|----------------------------------------------------------------------------------|
| seq payload stream                                                                |
|----------------------------------------------------------------------------------|
| qual payload stream                                                               |
+----------------------------------------------------------------------------------+

+----------------------------------------------------------------------------------+
| block 1                                                                          |
|   ...                                                                            |
+----------------------------------------------------------------------------------+
```

This layout keeps the container general enough for multiple transient workflows while still matching the main performance needs of sorting and partitioning.

## Why blocks, not serialized record blobs

`dryice` should not center the format on a stream like this:

```text
[record][record][record][record]...
```

That style is easy to prototype, but it works against the main goals. It makes it harder to:

- scan cheaply without full record materialization
- keep optional side arrays dense and cache-friendly
- separate the hot comparison path from the colder payload path
- apply different encodings to sequence and quality sections cleanly
- do blockwise transforms and blockwise IO efficiently

The format should therefore treat blocks as self-contained traversal units.

## Records and fields

The initial conceptual record is read-like and should assume, at minimum, support for:

- record name or identifier
- sequence
- quality string
- per-record flags or small metadata

The format should stay read-centric. If a future use case needs arbitrary genomic annotations or rich schema evolution, that should be treated as pressure away from the intended scope rather than something to absorb automatically.

## Record index table

Each block should have one fixed-width index entry per record. Conceptually, each entry points into the payload sections and carries small local metadata.

For example:

```text
name_off | name_len | seq_off | seq_len | qual_off | qual_len | flags
```

Exact widths are a later implementation decision, but the important design point is fixed-width per-record indexing within a block.

This gives `dryice`:

- predictable traversal
- constant-time access to a record's field slices within the block
- a natural place to hang small per-record metadata
- the ability to omit repeated lengths when a block-level invariant makes that worthwhile

## Payload streams

The current plan is to keep payloads separated by field type:

- names
- sequences
- qualities

This improves flexibility and keeps later transforms honest. Names are often inert for ordering, sequences benefit from genomic-aware encodings, and qualities have their own space-versus-throughput tradeoffs.

Keeping these streams separate also makes it easier to support workloads that only need to touch some of the fields in the hot path.

## Optional side arrays

One of the most promising ideas in the planning exchange is the use of optional accelerator arrays stored alongside the record data.

Examples include:

- sort key
- minimizer hash
- partition or bucket id
- duplicate marker
- read-pair linkage id
- other lightweight derived per-record values used repeatedly in a workflow

Conceptually:

```text
+--------------------------------------------------------------+
| block                                                        |
|--------------------------------------------------------------|
| record index                                                 |
| names                                                        |
| sequences                                                    |
| qualities                                                    |
|--------------------------------------------------------------|
| optional side arrays                                         |
|   sort_key[]                                                 |
|   minimizer[]                                                |
|   bucket_id[]                                                |
|   flags[]                                                    |
+--------------------------------------------------------------+
```

This is important because it keeps the design from becoming sorter-only without forcing every workflow to pay for features it does not use.

## Sequence encoding strategy

Sequence storage is a central design decision.

The current plan should assume at least three distinct sequence modes:

- `SeqRawAscii`
- `Seq2BitExact`
- `Seq2BitLossyN`

An optional `Seq4BitIupac` exact mode may also be defensible as a simpler fallback, but it should not displace the main exact compact representation.

### Fast mode: raw ASCII

Raw ASCII sequence storage should remain a first-class mode because it keeps encoding and decoding cheap and avoids unpacking overhead. On fast local storage, this may be the throughput winner.

### Exact compact mode: 2-bit plus ambiguity side channel

This should be treated as the main compact exact representation.

The conceptual layout is:

```text
2-bit canonical stream
+
ambiguity mask
+
ambiguity byte stream
```

Meaning:

- A, C, G, and T are stored in 2 bits each
- a mask marks positions that are ambiguous or noncanonical
- a separate ambiguity stream stores exact IUPAC symbols only for masked positions

ASCII sketch:

```text
sequence:      A   C   G   T   N   A   R   T
position:      0   1   2   3   4   5   6   7

2-bit bases:   00  01  10  11  --  00  --  11
ambig mask:    0   0   0   0   1   0   1   0
ambig chars:                   N       R
```

This is the best current candidate because it gives:

- exact reconstruction
- 2-bit density for the common case
- no need to inflate all bases to 4 bits because some are ambiguous
- a clean answer to the fact that IUPAC handling is table stakes for serious compact sequence support

### Lossy compact mode: ambiguity collapse

The main lossy sequence mode worth keeping in scope is ambiguity collapse to an `N`-like bucket before or during compact encoding.

This should stay explicitly optional and clearly marked as lossy. It is not the default design center, but it is a reasonable experimental mode for some temporary workflows.

## Quality encoding strategy

Quality strings deserve their own design track.

The first-pass plan should keep this simple:

- `QualRaw`
- `QualBinned`

Raw quality storage is the conservative, low-complexity default. Binned quality storage is the most promising early lossy transform because it is cheap, likely useful, and easier to reason about than aggressive sequence lossiness.

The main design principle is that quality transforms should be cheap enough that they do not undermine the throughput case for the format.

## Name handling

Names should be supported, but treated as colder data than sequence-derived keys and many sequence fields.

The initial plan can assume:

- raw name storage as the baseline
- possible lightweight structural tricks later, such as prefix-aware handling, if benchmarks justify them

Names should not dominate the design.

## Sorting and merge-oriented access

Even though `dryice` should not be sorter-only, sorting remains the best stress test for the format.

For k-way merge workloads, the desired access pattern looks like this:

```text
run A block                  run B block                  run C block
-----------                  -----------                  -----------
keys                         keys                         keys
[idx]                        [idx]                        [idx]
payloads                     payloads                     payloads

   |                            |                            |
   v                            v                            v

current key A              current key B               current key C
      \                        |                        /
       \                       |                       /
        +------------------ merge heap ----------------+
                            |
                            v
                     choose smallest key
                            |
                            v
                  use index entry to fetch payload
                            |
                            v
                      emit / rewrite output
```

This is why optional stored keys matter so much. The hot path should be able to touch dense key material first and leave colder payload decoding for later.

## Three initial operating modes

The planning exchange converged on a useful mental model with three broad modes:

```text
MODE 1: FAST
+--------------------------------------------------------------+
| names: raw | seq: raw ascii | qual: raw ascii | key: opt     |
+--------------------------------------------------------------+

MODE 2: EXACT COMPACT
+--------------------------------------------------------------+
| names: raw | seq: 2bit + ambig mask + ambig chars            |
| qual: raw or binned | key: opt                               |
+--------------------------------------------------------------+

MODE 3: LOSSY COMPACT
+--------------------------------------------------------------+
| names: raw | seq: ambiguity-collapsed compact mode           |
| qual: binned | key: opt                                      |
+--------------------------------------------------------------+
```

These are not necessarily the final public API modes, but they are a good first-pass way to think about the intended tradeoff space.

## Design principles

The format should continue to follow these principles unless benchmarking or implementation reality forces a change.

1. Optimize for total transient workflow throughput, not theoretical compactness alone.
2. Keep the hot path cheap for sequential write, sequential read, and merge-like access.
3. Treat exact compact sequence support as first-class, not speculative.
4. Keep lossy transforms optional and explicit.
5. Prefer simple, explainable encodings before clever codecs.
6. Keep sorting as the hardest benchmarked use case without reducing the format to a sorter-only artifact.
7. Preserve a read-centric model instead of drifting toward a general schema container.

## Decisions that feel solid already

These points appear stable enough to treat as first-pass design commitments.

- `dryice` is a transient genomic container, not an archival standard.
- It is for read-like records, not arbitrary genomics objects.
- The file should be block-oriented.
- Record indexing within a block should be explicit and fixed-width.
- Names, sequences, and qualities should live in separate payload sections.
- Optional side arrays are part of the design, especially for sort keys and similar accelerators.
- Raw ASCII and exact compact sequence modes both need first-class support.
- Phred binning is the most promising early lossy experiment.

## Questions to carry forward

These are format questions that should stay open for now.

- Which block sizes are the right starting point for the main workloads?
- Should side arrays live in a generic registry-style section model or a narrower built-in set at first?
- Is `Seq4BitIupac` worth carrying early, or should it wait until there is evidence it earns its keep?
- How much optional metadata belongs in the file header before the format starts drifting toward overdesign?
- Which invariants, if any, should be block-level rather than per-record to reduce index size?
- How should exact-versus-lossy mode choices be encoded so mistakes are hard to make?

## First-pass summary

`dryice` should be built around a simple idea: genomic records deserve a temporary storage format that is faster and more structurally appropriate than generic serialization, but narrower and less ambitious than a general persistent genomics format.

The best current shape is a block-oriented transient container for read-like genomic records with:

- explicit record indexing
- separate name, sequence, and quality payload streams
- optional accelerator arrays such as sort keys
- raw and compact exact sequence modes
- carefully bounded optional lossy modes

That gives the project a strong center without yet dragging in Rust-specific crate structure or implementation details.

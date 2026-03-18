# dryice roadmap

This document tracks future development priorities beyond the current core library. It is organized roughly by expected impact and readiness, not by a fixed timeline.

## Language wrappers

### Python wrapper (`dryice-python`)

A PyO3-based wrapper crate that exposes the core dryice reader/writer API to Python. This is probably the single highest-impact item on the roadmap because the bioinformatics community overwhelmingly works in Python.

The wrapper should provide:

- a `DryIceWriter` class with a builder-style configuration API
- a `DryIceReader` class that yields records as Python objects or integrates with common Python genomics types
- support for all built-in codecs and record key types
- ideally, integration with `numpy` or `pyarrow` for batch access to sequence/quality arrays
- a path toward interop with `pysam`, `biopython`, and other Python genomics libraries

The core Rust API was designed from the start with wrapper-friendliness in mind: owned types at the public boundary, explicit configuration, structured errors, and no casual lifetime leakage. This should make the PyO3 wrapper relatively straightforward.

### Node wrapper (`dryice-node`)

An NAPI-RS-based wrapper crate that exposes the core dryice API to TypeScript and JavaScript. Lower priority than Python but still valuable for bioinformatics web tools, serverless genomics pipelines, and Node-based workflow engines.

The wrapper should provide:

- `DryIceWriter` and `DryIceReader` classes
- support for built-in codecs and keys
- integration with Node `Buffer` and `ReadableStream` types

## Rust ecosystem adapters (`dryice-adapters`)

A separate workspace crate that provides feature-gated `SeqRecordLike` implementations for record types from popular Rust bioinformatics libraries. This bridges the gap between dryice's parser-agnostic core and the libraries users actually parse FASTQ/BAM/FASTA with.

Likely initial targets:

- `noodles-fastq` — the most actively maintained Rust FASTQ library
- `needletail` — popular for high-performance sequence scanning
- `bio` / `rust-bio` — the original Rust bioinformatics toolkit

Each adapter would be behind a cargo feature flag so users only pay for the dependencies they actually use:

```toml
[dependencies]
dryice-adapters = { version = "0.1", features = ["noodles"] }
```

The adapter crate should also provide convenience functions for common patterns like "read a FASTQ file and write it as dryice" or "convert a dryice file back to FASTQ."

## Real-world testing with SRA data

The current test suite uses synthetic records. Real-world validation requires testing with actual sequencing data from public repositories.

The plan is to build a test harness (possibly conda-based for dependency management) that:

- downloads real FASTQ data from NCBI SRA using `fasterq-dump` or similar tools
- writes the data through dryice with various codec configurations
- reads it back and verifies exact round-trip fidelity (for lossless codecs) or structural integrity (for lossy codecs)
- measures throughput on real data for comparison against the synthetic benchmarks
- tests edge cases that synthetic data may not cover: variable read lengths, unusual quality distributions, non-standard name formats, very long reads (nanopore), paired-end interleaving

This could also serve as a compelling demo: "download 10M reads from SRA, sort them globally by sequence using dryice as the spill format, and verify the result."

## Performance engineering

The primary goal of `dryice` is extremely high throughput. The first round of performance work has already delivered major improvements: eliminating per-record heap allocations via `encode_into`/`decode_into` codec traits and making `BlockBuilder` generic over codec types to eliminate function pointer indirection produced a 208% write throughput improvement and 128% round-trip improvement. The raw write path now runs at 15.7 GiB/s and the raw round-trip at 5.7 GiB/s — nearly 2x faster than FASTQ text.

This section tracks the ongoing effort to find, implement, test, and finalize further performance improvements that push the format toward bare-metal throughput. Some of these will come at the expense of readability or internal simplicity, and that's acceptable. The library's reason for existing is speed.

### Completed optimizations

- codec traits changed from `encode() -> Vec<u8>` to `encode_into(&mut Vec<u8>)`, eliminating 3 heap allocations per record on write and enabling buffer reuse on read
- `BlockBuilder` made generic over `S, Q, N` codec types, eliminating function pointer indirection and enabling full inlining of codec encode paths by the compiler
- allocating `encode()`/`decode()` convenience methods retained as default trait methods for ergonomic use outside the hot path

### Prior art and technique collection

Further optimization should study what the fastest serialization and I/O libraries actually do. Relevant prior art includes:

- `cap'n proto` and `flatbuffers` for zero-copy serialization patterns
- `arrow` and `parquet` for columnar batch I/O techniques
- `lz4` and `zstd` for fast compression integration patterns
- `io_uring` and `aio` for async I/O submission on Linux
- SIMD-accelerated parsing and encoding techniques (building on what `bitnuc` already provides)
- buffer pooling and arena allocation patterns to further reduce allocator pressure
- cache-line-aware data layout for hot-path structures

### Write path optimization

The write path now encodes directly into block-owned buffers via `encode_into` with no intermediate allocations. Remaining potential improvements include:

- vectored writes (`writev`) to reduce syscall overhead when flushing blocks
- block-level parallelism: encoding the next block on a background thread while the current one flushes
- pre-sizing block buffers based on estimated record sizes to reduce `Vec` growth reallocation

### Read path optimization

The read path now reuses decode buffers across records via `decode_into`. Remaining potential improvements include:

- lazy decoding: only decode a field when it's actually accessed, not on every `advance()`
- prefetching: start reading the next block while the current one is being consumed
- SIMD-accelerated index parsing for the fixed-width record index entries

### Codec-level optimization

Individual codecs may benefit from:

- lookup-table-based encoding/decoding instead of per-byte branching
- SIMD-accelerated ambiguity scanning in the 2-bit codecs
- batch quality binning using SIMD comparison and shuffle instructions
- exploring whether the sparse ambiguity sideband can be encoded more compactly for the common case of zero ambiguous bases

### Measurement discipline

Performance work must be measurement-driven. Every optimization should be benchmarked before and after with `criterion`, and regressions should be caught by CI. The benchmark suite should grow alongside the optimization work to cover:

- varying record sizes (short Illumina, long nanopore)
- varying ambiguity rates
- varying block sizes
- file-backed I/O (not just in-memory buffers)
- multi-threaded write/read scenarios

The goal is not to optimize everything at once but to maintain a continuous improvement loop: measure, identify the bottleneck, optimize it, verify the improvement, move on.

## Format and codec enhancements

### Block-level checksums

The block header already reserves space for checksum metadata, but no checksum implementation exists yet. Adding optional CRC32 or XXH3 checksums per block would provide data integrity verification for workflows where corruption detection matters (e.g., network-backed storage, long-running pipelines).

### Name encoding improvements

The `SplitNameCodec` is a good start, but Illumina-style names have highly redundant prefixes across reads in the same run. A prefix-deduplication codec that stores the common prefix once per block and only the varying suffix per record could substantially reduce name storage for Illumina data.

### Additional sequence codecs

The current codec set covers the most important cases, but there may be value in:

- a 4-bit IUPAC codec for data with very high ambiguity rates
- a codec optimized for long reads (nanopore/PacBio) where the sequence characteristics differ from short Illumina reads
- a codec that integrates lightweight compression (e.g., LZ4) for cases where compactness matters more than decode speed

## Infrastructure

### Async I/O support

`tokio::io::AsyncRead` and `AsyncWrite` variants of the reader and writer would matter for cloud-backed or networked storage scenarios. The core block-oriented architecture should make this relatively clean since blocks are natural units of async I/O. This should be gated behind a cargo feature (e.g., `async`) so that users who don't need async don't pay for the `tokio` dependency.

Design considerations: the core types (`BlockBuilder`, `BlockDecoder`, `DryIceWriter`, `DryIceReader`) are likely already `Send` since they hold owned buffers and function pointers, but this has not been formally verified. The zero-copy reader pattern (where the reader implements `SeqRecordLike` for the current record via borrowed slices) may create friction with async borrowing, since references to `reader.sequence()` live until the next `next_record()` call. Before starting async work, we should add `static_assertions` for `Send` and `Sync` on the core types to catch regressions early.

### Streaming support

The block-oriented architecture is a natural fit for streaming. Blocks are self-contained units that can be produced, transmitted, and consumed independently, which means a streaming API could let users write blocks to a channel, socket, or pipe as they fill up and read blocks from a stream as they arrive, yielding records before the full file is available.

This pairs naturally with async I/O and would generalize the spill/reload pattern beyond files to inter-process communication, network transfer, and pipeline stages connected by channels. A streaming reader could implement something like `futures::Stream<Item = Result<SeqRecord, DryIceError>>` or expose a block-at-a-time async iterator.

The main design questions are around backpressure (what happens when the consumer is slower than the producer), partial block handling (what if the stream closes mid-block), and whether the streaming API should be a separate type or a mode on the existing reader/writer. This should be designed alongside async I/O rather than independently.

### Memory-mapped reading

An `mmap`-based reader that maps block data directly from the OS page cache rather than reading into owned buffers. This could be faster for random-access patterns and would reduce memory pressure for very large files. The block-oriented layout is well-suited to this since blocks are self-contained and aligned.

Open design question: whether this should be feature-gated or always available. The `memmap2` dependency is small and platform-portable, so there may not be a strong reason to gate it. On the other hand, mmap semantics are subtly different from normal reads (signals on I/O errors, platform-specific behavior), so keeping it opt-in might be the more honest choice.

### Crates.io publishing

The core `dryice` crate should be published to crates.io once the API surface feels stable enough for a `0.1.0` release. This requires:

- finalizing the public API surface
- writing crate-level documentation suitable for docs.rs
- choosing a minimum supported Rust version (MSRV) policy
- setting up automated publishing in the release workflow

### Documentation site

An `mdbook` or similar documentation site that goes beyond API docs to cover:

- format specification in detail
- codec implementation guide
- integration patterns with common genomics workflows
- performance tuning guide (block sizes, codec selection, key design)
- migration guide for users coming from other temporary storage approaches

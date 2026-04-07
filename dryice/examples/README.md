# dryice examples

These examples demonstrate the primary workflows that `dryice` is designed to support. Each one is a standalone Rust program that can be run with `cargo run --example <name>`.

## kmer_keys

The flagship example for the new kmer-derived key families and their progressive-disclosure ergonomics. This example shows how built-in packed canonical key families for prefixes and minimizers compose with the normal keyed writer and reader APIs, while also demonstrating the builder conveniences that reduce type noise without changing the underlying storage model.

## key_only_kmers

This is the most compact kmer-oriented workflow currently supported by `dryice`: keep exactly one derived key per record and omit the row payload entirely. The example writes only minimizer keys to disk and reads them back with `next_key()`, demonstrating how the new empty-payload APIs support very small, laptop-friendly temporary files for later stages of a pipeline.

## kmer_name_pairs

This example shows that `dryice` now supports more than the two extremes of “full reads” and “key-only files”. It writes minimizer keys while retaining only record names, making the partial-payload story concrete and demonstrating how users can keep lightweight traceability without paying to persist full sequences or qualities.

## kmer_partitioning

This example upgrades the old first-base partitioning idea into a more bioinformatics-native flow by deriving packed canonical prefix kmer keys and using them to bucket records. Each bucket keeps names plus keys only, showing how real derived genomic features can drive temporary partitioning workflows directly.

## spill_reload

The most fundamental `dryice` pattern: spilling a batch of sequencing records to a temporary buffer and reloading them later. This is the building block for any out-of-core workflow where data needs to move to disk and back without paying FASTQ reparse costs. The example generates 100 synthetic records, spills them with configurable block sizes, and reloads them using both the zero-copy primary path and the owned-record convenience iterator, verifying exact round-trip fidelity.

## external_merge_sort

The flagship use case for `dryice` and the reason record keys exist. This example implements a complete external k-way merge sort of sequencing records that are too large to fit in memory. It works in two phases: first, records are read in RAM-sized chunks, each chunk is sorted by a precomputed 8-byte key derived from the sequence, and the sorted run is spilled to a `dryice` temp file with the keys stored alongside the records. Second, all sorted runs are opened simultaneously and merged using a min-heap that compares only the 8-byte keys — the full sequence and quality payloads are never touched during comparison. The winning record is piped to the output writer, and the result is verified to be in globally sorted order.

## partitioning

Many sequencing workflows need to group reads into buckets before further processing — for example, by minimizer, barcode, or some other derived criterion. This example partitions records into four buckets based on the first base of the sequence, writing each bucket to its own `dryice` buffer. It then reads each partition back and reports the record counts and sizes, demonstrating how `dryice` can serve as fast temporary storage for partitioning stages in larger pipelines.

## compact_codecs

`dryice` supports multiple sequence, quality, and name encodings that trade compactness for speed. This example writes the same 1,000 records twice — once with raw ASCII codecs and once with 2-bit exact sequence encoding, binned quality scores, and split name storage — and compares the resulting file sizes. It then reads the compact file back to verify round-trip fidelity, demonstrating that compact codecs are transparent to the reader once the correct codec types are specified.

## record_keys

Record keys are fixed-width accelerator values stored alongside each record in a `dryice` file. They are designed for workflows where records need to be compared or ordered by a derived value without touching the full payload. This example remains the generic foundation example: it writes four records with simple 8-byte keys computed from their sequences, then reads them back and prints each record's key, showing the underlying mechanism that the newer kmer-focused examples build on.

## zero_copy_pipe

One of `dryice`'s strongest properties is that the reader implements `SeqRecordLike`, which means it can be passed directly to the writer's `write_record` method with no intermediate allocation. This example writes 50 records to a source buffer, then pipes them through a reader into a destination writer with a different block size, demonstrating zero-copy record transfer between `dryice` files. The size difference between source and destination reflects only the block header overhead from the different blocking factor.

## custom_codec

The codec system in `dryice` is trait-based, which means users can implement their own sequence, quality, or name encodings. This example implements a simple run-length encoding codec for sequences with long homopolymer runs, writes records using it, and reads them back. It also compares the encoded size against raw storage to show the compression effect. The example demonstrates the full codec contract: `TYPE_TAG`, `LOSSY`, `encode_into`, and `decode_into`.

## noodles_adapter

The recommended pattern for integrating `dryice` with the [noodles](https://github.com/zaeleus/noodles) FASTQ library. Rather than depending on an adapter crate, users write a thin newtype wrapper with a `Deref` impl and a `SeqRecordLike` impl — about 15 lines of code. This keeps the user in control of which noodles version they use and avoids any semver coupling between `dryice` and noodles. The example parses FASTQ records with noodles, writes them into a `dryice` buffer through the wrapper, and reads them back.

## rust_bio_adapter

The same newtype + `Deref` + `SeqRecordLike` pattern applied to the [rust-bio](https://github.com/rust-bio/rust-bio) library. This demonstrates that the pattern works identically for any library that provides a FASTQ record type — the only thing that changes is the three method bodies mapping the library's field accessors to `dryice`'s `name()`, `sequence()`, and `quality()` interface.

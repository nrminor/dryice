---
name: dryice
description: High-throughput transient container for read-like genomic records. Use when writing Rust code that needs fast temporary disk persistence for sequencing data, including external sorting, spill/reload pipelines, partitioning, bucketing, or any out-of-core genomics workflow. Also use when the user mentions dryice, temporary FASTQ storage, sequence sorting, record keys, or genomic spill files.
license: MIT
metadata:
  repository: https://github.com/nrminor/dryice
---

# dryice

Block-oriented temporary storage format for sequencing records, optimized for high-throughput spill/reload workflows. The crate is parser-agnostic: any type implementing `SeqRecordLike` can be written, and records are read back as zero-copy borrowed slices.

The writer and reader are generic over five type parameters with sensible defaults: `DryIceWriter<W, S, Q, N, K>` and `DryIceReader<R, S, Q, N, K>`, where `S` is the sequence codec, `Q` is the quality codec, `N` is the name codec, and `K` is the optional record key type. Users never see these parameters unless they opt into non-default codecs or keys.

## Writing records (default codecs)

```rust
use dryice::{DryIceWriter, SeqRecord};

let mut buf = Vec::new();
let mut writer = DryIceWriter::builder()
    .inner(&mut buf)
    .build();

let record = SeqRecord::new(
    b"read1".to_vec(),
    b"ACGTACGT".to_vec(),
    b"!!!!!!!!".to_vec(),
)?;
writer.write_record(&record)?;
writer.finish()?;
```

## Writing with compact codecs

The builder uses typestate transitions for codec and key selection. Each transition method consumes the builder and returns a new one with the updated type parameter.

```rust
use dryice::DryIceWriter;

let mut writer = DryIceWriter::builder()
    .inner(file)
    .two_bit_exact()       // SequenceCodec -> TwoBitExactCodec
    .binned_quality()      // QualityCodec -> BinnedQualityCodec
    .split_names()         // NameCodec -> SplitNameCodec
    .target_block_records(4096)
    .build();
```

Available builder convenience methods for codecs:

- `.two_bit_exact()` — SIMD-accelerated 2-bit packing with exact IUPAC reconstruction
- `.two_bit_lossy_n()` — 2-bit packing that collapses all ambiguous bases to N
- `.binned_quality()` — Illumina-style 8-level Phred binning
- `.omit_quality()` — drop quality scores entirely
- `.split_names()` — split names on first space into id and description
- `.omit_names()` — drop names entirely

For user-defined codecs: `.sequence_codec::<S>()`, `.quality_codec::<Q>()`, `.name_codec::<N>()`.

## Writing with record keys

Record keys are fixed-width accelerator values stored alongside records for fast comparison without touching payloads.

```rust
use dryice::{Bytes8Key, DryIceWriter, SeqRecord};

let mut writer = DryIceWriter::builder()
    .inner(file)
    .bytes8_key()          // RecordKey -> Bytes8Key
    .build();

let record = SeqRecord::new(b"r1".to_vec(), b"ACGT".to_vec(), b"!!!!".to_vec())?;
let key = Bytes8Key(*b"sortkey!");
writer.write_record_with_key(&record, &key)?;
writer.finish()?;
```

Built-in key convenience methods: `.bytes8_key()`, `.bytes16_key()`. For user-defined keys: `.record_key::<K>()`.

## Reading records (zero-copy primary path)

The reader itself implements `SeqRecordLike` for the current record. After `next_record()` returns `true`, call `name()`, `sequence()`, and `quality()` on the reader for zero-copy borrowed slices into block-owned buffers.

```rust
use dryice::{DryIceReader, SeqRecordLike};

let mut reader = DryIceReader::new(file)?;
while reader.next_record()? {
    let seq = reader.sequence();   // &[u8], no allocation
    let qual = reader.quality();   // &[u8], no allocation
}
```

## Reading records (convenience iterator)

For `for`-loop ergonomics, `into_records()` yields owned `SeqRecord` values. This allocates per record.

```rust
use dryice::DryIceReader;

let reader = DryIceReader::new(file)?;
for record in reader.into_records() {
    let record = record?;
    println!("{}", record);
}
```

## Reading with non-default codecs

The reader must be opened with the same codec types used to write the file. Mismatches are caught at block-load time with clear error messages.

```rust
use dryice::{BinnedQualityCodec, DryIceReader, SplitNameCodec, TwoBitExactCodec};

let mut reader = DryIceReader::with_codecs::<
    TwoBitExactCodec,
    BinnedQualityCodec,
    SplitNameCodec,
>(file)?;
```

For keyed readers: `DryIceReader::with_bytes8_key(file)`, `DryIceReader::with_record_key::<K>(file)`.

## Reading record keys

```rust
use dryice::{Bytes8Key, DryIceReader};

let mut reader = DryIceReader::with_bytes8_key(file)?;
while reader.next_record()? {
    let key: Bytes8Key = reader.record_key()?;
}
```

## Zero-copy reader-to-writer piping

Because the reader implements `SeqRecordLike`, it can be passed directly to the writer with no intermediate allocation:

```rust
while reader.next_record()? {
    writer.write_record(&reader)?;
}
```

## The SeqRecordLike trait

The write-side interoperability boundary. Implement this on your own record types to write them into dryice files without conversion:

```rust
use dryice::SeqRecordLike;

impl SeqRecordLike for MyRecord {
    fn name(&self) -> &[u8] { &self.name }
    fn sequence(&self) -> &[u8] { &self.seq }
    fn quality(&self) -> &[u8] { &self.qual }
}
```

The trait also provides default `len()` and `is_empty()` methods. The `SeqRecordExt` extension trait adds `to_seq_record()` for converting any `SeqRecordLike` implementor into an owned `SeqRecord`.

## Implementing custom codecs

Sequence and quality codecs implement `SequenceCodec` or `QualityCodec`:

```rust
use dryice::{DryIceError, SequenceCodec};

struct MyCodec;

impl SequenceCodec for MyCodec {
    const TYPE_TAG: [u8; 16] = *b"myorg:seq:custom";
    const LOSSY: bool = false;

    fn encode(sequence: &[u8]) -> Result<Vec<u8>, DryIceError> { /* ... */ }
    fn decode(encoded: &[u8], original_len: usize) -> Result<Vec<u8>, DryIceError> { /* ... */ }
}
```

Name codecs have an associated `Decoded` type for richer parsed representations:

```rust
use dryice::{DryIceError, NameCodec};

struct MyNameCodec;
struct MyDecodedName { /* parsed fields */ }

impl NameCodec for MyNameCodec {
    const TYPE_TAG: [u8; 16] = *b"myorg:name:custm";
    const LOSSY: bool = false;
    type Decoded = MyDecodedName;

    fn encode(name: &[u8]) -> Result<Vec<u8>, DryIceError> { /* ... */ }
    fn decode(encoded: &[u8], original_len: usize) -> Result<MyDecodedName, DryIceError> { /* ... */ }
    fn as_bytes(decoded: &MyDecodedName) -> &[u8] { /* ... */ }
}
```

Record keys implement `RecordKey`:

```rust
use dryice::{DryIceError, RecordKey};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct MyKey([u8; 12]);

impl RecordKey for MyKey {
    const WIDTH: u16 = 12;
    const TYPE_TAG: [u8; 16] = *b"myorg:key:custom";

    fn encode_into(&self, out: &mut [u8]) { out.copy_from_slice(&self.0); }
    fn decode_from(bytes: &[u8]) -> Result<Self, DryIceError> {
        Ok(Self(bytes.try_into().map_err(|_| DryIceError::InvalidRecordKeyEncoding {
            message: "wrong key length",
        })?))
    }
}
```

## Error handling

All fallible operations return `Result<T, DryIceError>`. Key error variants:

- `DryIceError::Io` — underlying I/O failure
- `DryIceError::InvalidMagic` — file doesn't start with `DRYI`
- `DryIceError::MismatchedSequenceAndQualityLengths` — record validation failure
- `DryIceError::SequenceCodecMismatch` / `QualityCodecMismatch` / `NameCodecMismatch` — reader opened with wrong codec type
- `DryIceError::RecordKeyTypeMismatch` — reader opened with wrong key type
- `DryIceError::CorruptBlockHeader` / `CorruptBlockLayout` / `CorruptRecordIndex` — format corruption

## Common patterns

**External merge sort:** Write sorted runs with record keys, then merge using a min-heap that compares only keys. See `examples/external_merge_sort.rs`.

**Temporary partitioning:** Write records into separate dryice buffers by bucket, reload each partition later. See `examples/partitioning.rs`.

**Format conversion pipeline:** Read from one dryice file, pipe zero-copy to a writer with different codecs or block sizes. See `examples/zero_copy_pipe.rs`.

## What dryice is NOT for

- Long-term archival storage (use BAM/CRAM/FASTQ)
- General-purpose columnar analytics (use Arrow/Parquet)
- Cross-ecosystem data exchange (use standard formats)
- Arbitrary genomic transformations (use specialized tools)

dryice is for fast temporary persistence of sequencing records in out-of-core workflows.

# dryice-node

Node.js/TypeScript bindings for [dryice](https://github.com/nrminor/dryice), a high-throughput transient container for read-like genomic records.

## Installation

> [!NOTE]
> Publishing to npm is coming soon. For now, install from source.

```sh
cd dryice-node
bun install
bun run build
```

## Quick start

### Writing records

```typescript
import { WriterBuilder } from "./api.js";

const writer = new WriterBuilder().build();
writer.writeRecord(
  Buffer.from("read1"),
  Buffer.from("ACGTACGT"),
  Buffer.from("!!!!!!!!")
);
const data = writer.finish();
```

### Reading records

```typescript
import { Reader } from "./api.js";

const reader = Reader.open(data);
const records = reader.records();
for (const record of records) {
  console.log(record.name, record.sequence);
}
```

### Temporary files

For spill/reload workflows, let `dryice` create the temporary file and clean it up when the workflow is done:

```typescript
import { ReaderBuilder, WriterBuilder, withTempFile } from "./api.js";

const records = withTempFile((tmp) => {
  const writer = new WriterBuilder().buildTemp(tmp);
  writer.writeRecord(Buffer.from("read1"), Buffer.from("ACGT"), Buffer.from("!!!!"));
  writer.finish();

  return new ReaderBuilder().buildTemp(tmp).records();
});
```

`withTempFile(...)` cleans up after either a synchronous callback returns or an async callback settles. If you need explicit control instead, use `tempFile()` with `try`/`finally` and call `tmp.cleanup()` yourself. Use `tmp.persist(path)` to move the file to a caller-owned location and disable automatic cleanup for that destination.

### Selective decoding

```typescript
import { ReaderBuilder } from "./api.js";

const reader = new ReaderBuilder()
  .twoBitExact()
  .binnedQuality()
  .splitNames()
  .bytes8Key()
  .select("sequence", "key")
  .build(data);

const record = reader.nextRecord();
if (record) {
  console.log(record.sequence, record.key);
  console.log("name" in record); // false at runtime
}
```

Selective decoding changes which fields are decoded for each record. `dryice` still reads full blocks from disk, but projected readers only decode the projection you ask for. In the handwritten TypeScript facade, `select(...)` also narrows the returned record type so omitted fields disappear from the static type.

### Compact codecs

```typescript
const writer = new WriterBuilder()
  .twoBitExact()
  .binnedQuality()
  .splitNames()
  .build();
```

### Record keys

```typescript
const writer = new WriterBuilder().bytes8Key().build();
writer.writeRecordWithKey(
  Buffer.from("r1"),
  Buffer.from("ACGT"),
  Buffer.from("!!!!"),
  Buffer.from("sortkey!")
);
const data = writer.finish();

const reader = new ReaderBuilder().bytes8Key().build(data);
const records = reader.records();
console.log(records[0].key);
```

### Kmer-oriented conveniences

```typescript
import {
  WriterBuilder,
  defaultMinimizerKey,
  defaultPrefixKmerKey,
} from "./api.js";

const sequence = Buffer.from("ACGTGCTCAGAGACTCAGAGGATTACAGTTTACGTGCTCAGAGACTCAGAGGA");
const key = defaultMinimizerKey(sequence);

const writer = new WriterBuilder()
  .minimizersWithNames()
  .build();

if (key) {
  writer.writeRecordWithKey(Buffer.from("read1"), Buffer.from(""), Buffer.from(""), key);
}
```

The kmer-oriented builder presets are wrapper-friendly shorthand for common key + payload-shaping choices:

- `prefixKmers()` / `prefixKmersWithSequences()` / `prefixKmersWithNames()`
- `minimizers()` / `minimizersWithSequences()` / `minimizersWithNames()`

The helper functions `defaultPrefixKmerKey(...)` and `defaultMinimizerKey(...)` return the packed 8-byte default key representations directly as `Buffer | null` so Node/TypeScript users do not need to work with the Rust key types themselves.

## API

The package uses a handwritten public TypeScript facade layered on top of the generated NAPI bindings. The main types are:

- `Writer` / `WriterBuilder` — write records with configurable codecs and keys
- `TempFile` / `TempWriter` — file-backed temporary workflows that clean up explicitly or through `withTempFile(...)`
- `Reader` / `ReaderBuilder` — read records with codec verification and optional selective decoding
- `ProjectedRecord<F>` — a projection-aware record type used by the TypeScript facade for `select(...)`
- `defaultPrefixKmerKey` / `defaultMinimizerKey` — helper functions for the built-in default packed kmer key representations

## About dryice

dryice is a block-oriented temporary storage format optimized for workflows where sequencing records need to move to disk and back quickly. It supports multiple sequence, quality, and name encodings, optional record keys for accelerated sorting, and zero-copy reads in the core Rust library.

For the full project documentation, see the [main repository](https://github.com/nrminor/dryice).

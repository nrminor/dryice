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
import { Writer } from "./index.js";

const writer = Writer.builder().build();
writer.writeRecord(
  Buffer.from("read1"),
  Buffer.from("ACGTACGT"),
  Buffer.from("!!!!!!!!")
);
const data = writer.finish();
```

### Reading records

```typescript
import { Reader } from "./index.js";

const reader = Reader.open(data);
const records = reader.records();
for (const record of records) {
  console.log(record.name, record.sequence);
}
```

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

## API

TypeScript type definitions are auto-generated from the Rust source via NAPI-RS. The main types are:

- `Writer` / `WriterBuilder` — write records with configurable codecs and keys
- `Reader` / `ReaderBuilder` — read records with codec verification
- `Record` — a decoded record with `name`, `sequence`, `quality`, and optional `key` fields (all `Buffer`)

## About dryice

dryice is a block-oriented temporary storage format optimized for workflows where sequencing records need to move to disk and back quickly. It supports multiple sequence, quality, and name encodings, optional record keys for accelerated sorting, and zero-copy reads in the core Rust library.

For the full project documentation, see the [main repository](https://github.com/nrminor/dryice).

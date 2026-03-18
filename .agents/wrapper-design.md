# dryice wrapper library design

This document captures the design direction for Python and Node wrapper libraries around the core `dryice` Rust crate.

## Guiding principle

The wrappers should expose all built-in functionality but should not expose trait-based extensibility. Custom codecs and record keys are a Rust-specific power that depends on the type system, monomorphization, and static dispatch. Trying to expose "implement your own codec" across FFI would be awkward and a maintenance burden.

The dividing line:

```text
exposed to Python/Node:
- DryIceWriter with builder configuration
- all built-in codec selections
- all built-in key types (Bytes8Key, Bytes16Key)
- DryIceReader with record iteration
- SeqRecord as a data object
- all error types
- record key access

not exposed to Python/Node:
- SequenceCodec trait
- QualityCodec trait
- NameCodec trait
- RecordKey trait
- SeqRecordLike trait
- custom codec/key type parameterization
```

## Tooling

### Python

- **PyO3** for the Rust-to-Python bindings
- **maturin** for building and packaging the Python extension module
- **uv** for Python project and dependency management

The crate would live at `dryice-python/` in the workspace.

### Node

- **NAPI-RS** for the Rust-to-Node bindings
- **TypeScript** for type definitions and the published package interface
- **Bun** for development, testing, and building
- the published package should work across Node runtimes (Node.js, Bun, Deno)

The crate would live at `dryice-node/` in the workspace.

## Internal dispatch strategy

In the Rust API, codec selection happens at the type level via generic parameters. In Python and Node, there are no type parameters. The wrapper layer needs to map runtime codec choices to the correct Rust generic instantiation internally.

The recommended approach is an internal enum dispatch layer inside each wrapper crate. The FFI boundary already has overhead (Python/Node function call costs dwarf the cost of a vtable lookup), so dynamic dispatch or enum matching inside the wrapper is fine.

Rather than enumerating every possible combination of codecs and keys (which is combinatorial), the wrapper should support a practical set of common configurations and dispatch to the correct monomorphized Rust type internally. Additional combinations can be added on demand.

An alternative is a thin dynamic-dispatch layer inside the core crate, feature-gated behind something like `dynamic`, that provides a `DynamicWriter` / `DynamicReader` not part of the public Rust API but available for wrapper use. This would avoid duplicating the dispatch logic across Python and Node wrappers.

## Python API design

### Writer builder

```python
from dryice import Writer

writer = (
    Writer.builder()
    .inner("reads.dryice")
    .two_bit_exact()
    .binned_quality()
    .split_names()
    .target_block_records(4096)
    .build()
)
```

The builder mirrors the Rust API. Each method returns the builder for chaining. The typestate transitions don't change the return type in Python (it's always `WriterBuilder`), but the runtime behavior is the same.

The minimal path stays simple:

```python
writer = Writer.builder().inner("reads.dryice").build()
```

### Writer operations

```python
writer.write_record(name=b"read1", sequence=b"ACGT", quality=b"!!!!")

writer.write_record_with_key(
    name=b"read1",
    sequence=b"ACGT",
    quality=b"!!!!",
    key=b"sortkey!",
)

writer.finish()
```

### Context manager support

```python
with Writer.builder().inner("reads.dryice").build() as writer:
    writer.write_record(name=b"r1", sequence=b"ACGT", quality=b"!!!!")
```

The context manager calls `finish()` automatically on exit.

### Reader

```python
from dryice import Reader

reader = Reader.open("reads.dryice")
for record in reader:
    print(record.name)
    print(record.sequence)
    print(record.quality)
```

### Reader with non-default codecs

```python
reader = (
    Reader.builder()
    .inner("reads.dryice")
    .two_bit_exact()
    .binned_quality()
    .split_names()
    .build()
)
```

### Reader with record keys

```python
reader = (
    Reader.builder()
    .inner("reads.dryice")
    .bytes8_key()
    .build()
)

for record in reader:
    print(record.key)
```

### Python type stubs

Type stubs should be provided for full IDE support:

```python
class WriterBuilder:
    def inner(self, path: str | PathLike | BinaryIO) -> WriterBuilder: ...
    def two_bit_exact(self) -> WriterBuilder: ...
    def two_bit_lossy_n(self) -> WriterBuilder: ...
    def binned_quality(self) -> WriterBuilder: ...
    def omit_quality(self) -> WriterBuilder: ...
    def split_names(self) -> WriterBuilder: ...
    def omit_names(self) -> WriterBuilder: ...
    def bytes8_key(self) -> WriterBuilder: ...
    def bytes16_key(self) -> WriterBuilder: ...
    def target_block_records(self, n: int) -> WriterBuilder: ...
    def build(self) -> Writer: ...

class Writer:
    @staticmethod
    def builder() -> WriterBuilder: ...
    def write_record(self, name: bytes, sequence: bytes, quality: bytes) -> None: ...
    def write_record_with_key(self, name: bytes, sequence: bytes, quality: bytes, key: bytes) -> None: ...
    def finish(self) -> None: ...
    def __enter__(self) -> Writer: ...
    def __exit__(self, *args: Any) -> None: ...

class Record:
    @property
    def name(self) -> bytes: ...
    @property
    def sequence(self) -> bytes: ...
    @property
    def quality(self) -> bytes: ...
    @property
    def key(self) -> bytes | None: ...

class ReaderBuilder:
    def inner(self, path: str | PathLike | BinaryIO) -> ReaderBuilder: ...
    def two_bit_exact(self) -> ReaderBuilder: ...
    def two_bit_lossy_n(self) -> ReaderBuilder: ...
    def binned_quality(self) -> ReaderBuilder: ...
    def omit_quality(self) -> ReaderBuilder: ...
    def split_names(self) -> ReaderBuilder: ...
    def omit_names(self) -> ReaderBuilder: ...
    def bytes8_key(self) -> ReaderBuilder: ...
    def bytes16_key(self) -> ReaderBuilder: ...
    def build(self) -> Reader: ...

class Reader:
    @staticmethod
    def open(path: str | PathLike) -> Reader: ...
    @staticmethod
    def builder() -> ReaderBuilder: ...
    def __iter__(self) -> Iterator[Record]: ...
```

## Node/TypeScript API design

### Writer builder

```typescript
import { Writer } from 'dryice';

const writer = Writer.builder()
    .inner("reads.dryice")
    .twoBitExact()
    .binnedQuality()
    .splitNames()
    .targetBlockRecords(4096)
    .build();
```

### Writer operations

```typescript
writer.writeRecord({
    name: Buffer.from("read1"),
    sequence: Buffer.from("ACGT"),
    quality: Buffer.from("!!!!"),
});

writer.writeRecordWithKey({
    name: Buffer.from("read1"),
    sequence: Buffer.from("ACGT"),
    quality: Buffer.from("!!!!"),
    key: Buffer.from("sortkey!"),
});

writer.finish();
```

### Reader

```typescript
import { Reader } from 'dryice';

const reader = Reader.open("reads.dryice");
for await (const record of reader) {
    console.log(record.sequence);
}
```

### Reader with builder

```typescript
const reader = Reader.builder()
    .inner("reads.dryice")
    .twoBitExact()
    .binnedQuality()
    .bytes8Key()
    .build();
```

### TypeScript type definitions

TypeScript can express some typestate via conditional types:

```typescript
interface WriterBuilder<HasInner extends boolean = false> {
    inner(path: string): WriterBuilder<true>;
    twoBitExact(): WriterBuilder<HasInner>;
    twoBitLossyN(): WriterBuilder<HasInner>;
    binnedQuality(): WriterBuilder<HasInner>;
    omitQuality(): WriterBuilder<HasInner>;
    splitNames(): WriterBuilder<HasInner>;
    omitNames(): WriterBuilder<HasInner>;
    bytes8Key(): WriterBuilder<HasInner>;
    bytes16Key(): WriterBuilder<HasInner>;
    targetBlockRecords(n: number): WriterBuilder<HasInner>;
    build(this: WriterBuilder<true>): Writer;
}

interface Writer {
    writeRecord(record: RecordInput): void;
    writeRecordWithKey(record: RecordInputWithKey): void;
    finish(): void;
}

interface RecordInput {
    name: Buffer;
    sequence: Buffer;
    quality: Buffer;
}

interface RecordInputWithKey extends RecordInput {
    key: Buffer;
}

interface Record {
    readonly name: Buffer;
    readonly sequence: Buffer;
    readonly quality: Buffer;
    readonly key: Buffer | null;
}

interface ReaderBuilder<HasInner extends boolean = false> {
    inner(path: string): ReaderBuilder<true>;
    twoBitExact(): ReaderBuilder<HasInner>;
    twoBitLossyN(): ReaderBuilder<HasInner>;
    binnedQuality(): ReaderBuilder<HasInner>;
    omitQuality(): ReaderBuilder<HasInner>;
    splitNames(): ReaderBuilder<HasInner>;
    omitNames(): ReaderBuilder<HasInner>;
    bytes8Key(): ReaderBuilder<HasInner>;
    bytes16Key(): ReaderBuilder<HasInner>;
    build(this: ReaderBuilder<true>): Reader;
}

interface Reader {
    [Symbol.asyncIterator](): AsyncIterableIterator<Record>;
}
```

The `build(this: WriterBuilder<true>)` pattern means TypeScript will error at compile time if `.build()` is called without first calling `.inner()`.

## Error handling

### Python

Errors should be translated to a `DryIceError` exception class with structured attributes:

```python
try:
    reader = Reader.open("bad_file.dryice")
except dryice.DryIceError as e:
    print(e.kind)     # "InvalidMagic", "SequenceCodecMismatch", etc.
    print(e.message)
```

### Node

Errors should be translated to a `DryIceError` class extending `Error`:

```typescript
try {
    const reader = Reader.open("bad_file.dryice");
} catch (e) {
    if (e instanceof DryIceError) {
        console.log(e.kind);
        console.log(e.message);
    }
}
```

## Open design questions

- should the Python wrapper accept `numpy` arrays for batch sequence/quality access?
- should the Node wrapper support `ReadableStream` / `WritableStream` in addition to file paths?
- should the wrappers expose block-level metadata (record count, codec tags) for introspection?
- should `Record` in Python be a dataclass, a named tuple, or a custom class with `__slots__`?
- should the dynamic dispatch layer live in the core crate (feature-gated) or be duplicated in each wrapper?

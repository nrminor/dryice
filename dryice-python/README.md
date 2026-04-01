# dryice-python

Python bindings for [dryice](https://github.com/nrminor/dryice), a high-throughput transient container for read-like genomic records.

## Installation

> [!NOTE]
> Publishing to PyPI is coming soon. For now, install from source.

```sh
cd dryice-python
.venv/bin/maturin develop
```

## Quick start

### Writing records

```python
import dryice_python as di

writer = di.Writer.builder().build()
writer.write_record(b"read1", b"ACGTACGT", b"!!!!!!!!")
writer.write_record(b"read2", b"TGCATGCA", b"########")
data = writer.finish()
```

### Reading records

```python
reader = di.Reader.open(data)
for record in reader:
    print(record.name, record.sequence)
```

### Selective decoding

```python
reader = di.open_projected(
    data,
    "sequence+key",
    sequence_codec="two_bit_exact",
    quality_codec="binned",
    name_codec="split",
    record_key="bytes8",
)

for record in reader:
    print(record.sequence, record.key)
    assert record.name is None
    assert record.quality is None
```

Selective decoding changes which fields are decoded for each record. `dryice` still reads full blocks from disk, but projected readers only materialize the projection you ask for. In Python, unselected fields are exposed as `None`.

### Compact codecs

```python
writer = (
    di.WriterBuilder()
    .two_bit_exact()
    .binned_quality()
    .split_names()
    .build()
)
```

### Record keys

```python
writer = di.WriterBuilder().bytes8_key().build()
writer.write_record_with_key(b"r1", b"ACGT", b"!!!!", b"sortkey!")
data = writer.finish()

reader = di.ReaderBuilder().bytes8_key().build(data)
for record in reader:
    print(record.key)
```

## BioPython integration

See [examples/biopython_integration.py](examples/biopython_integration.py) for a complete example of converting between BioPython `SeqRecord` objects and dryice.

## API reference

The package ships with type stubs (`dryice_python.pyi`) for full IDE support. The main classes are:

- `Writer` / `WriterBuilder` — write records with configurable codecs and keys
- `Reader` / `ReaderBuilder` — iterate over records with codec verification
- `open_projected(...)` — open a reader with selective decoding for a supported projection
- `Record` — a decoded record with `name`, `sequence`, `quality`, and optional `key` fields; unselected fields are `None` in projected reads

## About dryice

dryice is a block-oriented temporary storage format optimized for workflows where sequencing records need to move to disk and back quickly. It supports multiple sequence, quality, and name encodings, optional record keys for accelerated sorting, and zero-copy reads in the core Rust library.

For the full project documentation, see the [main repository](https://github.com/nrminor/dryice).

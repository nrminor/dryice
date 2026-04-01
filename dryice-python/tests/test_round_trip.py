"""Round-trip tests for the dryice Python bindings."""

import dryice_python as di


def test_write_and_read_default_codecs():
    """Write records with default codecs and read them back."""
    writer = di.Writer.builder().build()
    writer.write_record(b"read1", b"ACGTACGT", b"!!!!!!!!")
    writer.write_record(b"read2", b"TGCATGCA", b"########")
    data = writer.finish()

    reader = di.Reader.open(data)
    records = list(reader)

    assert len(records) == 2
    assert records[0].name == b"read1"
    assert records[0].sequence == b"ACGTACGT"
    assert records[0].quality == b"!!!!!!!!"
    assert records[1].name == b"read2"
    assert records[1].sequence == b"TGCATGCA"
    assert records[1].quality == b"########"


def test_write_and_read_two_bit_exact():
    """Write with TwoBitExact codec and read back."""
    writer = di.WriterBuilder().two_bit_exact().build()
    writer.write_record(b"r1", b"ACNGT", b"!!!!!")
    data = writer.finish()

    reader = di.ReaderBuilder().two_bit_exact().build(data)
    records = list(reader)

    assert len(records) == 1
    assert records[0].name == b"r1"
    assert records[0].sequence == b"ACNGT"
    assert records[0].quality == b"!!!!!"


def test_write_and_read_compact():
    """Write with full compact codecs and read back."""
    writer = di.WriterBuilder().two_bit_exact().binned_quality().split_names().build()
    writer.write_record(
        b"instrument:run:flowcell 1:N:0:ATCACG", b"ACGTACGT", b"!!!!!!!!"
    )
    data = writer.finish()

    reader = (
        di.ReaderBuilder().two_bit_exact().binned_quality().split_names().build(data)
    )
    records = list(reader)

    assert len(records) == 1
    assert records[0].sequence == b"ACGTACGT"
    assert len(records[0].quality) == 8


def test_write_and_read_with_key():
    """Write with record keys and read them back."""
    writer = di.WriterBuilder().bytes8_key().build()
    writer.write_record_with_key(b"r1", b"ACGT", b"!!!!", b"sortkey!")
    writer.write_record_with_key(b"r2", b"TGCA", b"####", b"sortkey2")
    data = writer.finish()

    reader = di.ReaderBuilder().bytes8_key().build(data)
    records = list(reader)

    assert len(records) == 2
    assert records[0].key == b"sortkey!"
    assert records[1].key == b"sortkey2"


def test_select_sequence_projection():
    writer = di.WriterBuilder().two_bit_exact().binned_quality().split_names().build()
    writer.write_record(b"read1 desc", b"ACGTACGT", b"!!!!!!!!")
    data = writer.finish()

    reader = di.open_projected(
        data,
        "sequence",
        sequence_codec="two_bit_exact",
        quality_codec="binned",
        name_codec="split",
    )
    record = next(iter(reader))

    assert record.sequence == b"ACGTACGT"
    assert record.name is None
    assert record.quality is None
    assert record.key is None


def test_select_quality_projection():
    writer = di.WriterBuilder().two_bit_exact().binned_quality().split_names().build()
    writer.write_record(b"read1 desc", b"ACGTACGT", b"!!!!!!!!")
    data = writer.finish()

    reader = di.open_projected(
        data,
        "quality",
        sequence_codec="two_bit_exact",
        quality_codec="binned",
        name_codec="split",
    )
    record = next(iter(reader))

    assert record.quality is not None
    assert len(record.quality) == 8
    assert record.name is None
    assert record.sequence is None
    assert record.key is None


def test_select_name_projection():
    writer = di.WriterBuilder().two_bit_exact().binned_quality().split_names().build()
    writer.write_record(b"read1 desc", b"ACGTACGT", b"!!!!!!!!")
    data = writer.finish()

    reader = di.open_projected(
        data,
        "name",
        sequence_codec="two_bit_exact",
        quality_codec="binned",
        name_codec="split",
    )
    record = next(iter(reader))

    assert record.name == b"read1 desc"
    assert record.sequence is None
    assert record.quality is None
    assert record.key is None


def test_select_sequence_and_key_projection():
    writer = (
        di.WriterBuilder()
        .two_bit_exact()
        .binned_quality()
        .split_names()
        .bytes8_key()
        .build()
    )
    writer.write_record_with_key(b"read1 desc", b"ACGTACGT", b"!!!!!!!!", b"sortkey!")
    data = writer.finish()

    reader = di.open_projected(
        data,
        "sequence+key",
        sequence_codec="two_bit_exact",
        quality_codec="binned",
        name_codec="split",
        record_key="bytes8",
    )
    record = next(iter(reader))

    assert record.sequence == b"ACGTACGT"
    assert record.key == b"sortkey!"
    assert record.name is None
    assert record.quality is None


def test_select_rejects_unknown_field():
    try:
        di.open_projected(b"", "banana")
        assert False, "should have raised"
    except ValueError:
        pass


def test_empty_file():
    """Write and read an empty file."""
    writer = di.Writer.builder().build()
    data = writer.finish()

    reader = di.Reader.open(data)
    records = list(reader)

    assert len(records) == 0


def test_many_records():
    """Write and read many records to exercise block boundaries."""
    writer = di.WriterBuilder().target_block_records(10).build()
    for i in range(100):
        name = f"read_{i}".encode()
        seq = b"ACGTACGT"
        qual = b"!!!!!!!!"
        writer.write_record(name, seq, qual)
    data = writer.finish()

    reader = di.Reader.open(data)
    records = list(reader)

    assert len(records) == 100
    for i, record in enumerate(records):
        assert record.name == f"read_{i}".encode()
        assert record.sequence == b"ACGTACGT"


def test_record_repr():
    """Record has a useful repr."""
    writer = di.Writer.builder().build()
    writer.write_record(b"my_read", b"ACGT", b"!!!!")
    data = writer.finish()

    reader = di.Reader.open(data)
    record = next(iter(reader))

    assert "my_read" in repr(record)
    assert "4" in repr(record)


def test_writer_rejects_after_finish():
    """Writer raises after finish() is called."""
    writer = di.Writer.builder().build()
    writer.finish()

    try:
        writer.write_record(b"r1", b"ACGT", b"!!!!")
        assert False, "should have raised"
    except RuntimeError:
        pass


def test_writer_rejects_double_finish():
    """Writer raises on second finish() call."""
    writer = di.Writer.builder().build()
    writer.finish()

    try:
        writer.finish()
        assert False, "should have raised"
    except RuntimeError:
        pass

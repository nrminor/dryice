"""Example: using dryice with BioPython SeqRecord objects.

This example demonstrates how to convert between BioPython's SeqRecord
type and dryice's Writer/Reader, showing how dryice can serve as fast
temporary storage in a BioPython-based pipeline.

Usage:
    cd dryice-python
    uv run python examples/biopython_integration.py
"""

from Bio.Seq import Seq
from Bio.SeqRecord import SeqRecord

import dryice_python as di


def bio_to_dryice(bio_record: SeqRecord) -> tuple[bytes, bytes, bytes]:
    """Convert a BioPython SeqRecord to dryice field bytes."""
    name = bio_record.id.encode()
    sequence = bytes(bio_record.seq)
    quality = bytes(q + 33 for q in bio_record.letter_annotations.get("phred_quality", []))
    return name, sequence, quality


def dryice_to_bio(record: di.Record) -> SeqRecord:
    """Convert a dryice Record back to a BioPython SeqRecord."""
    seq = Seq(record.sequence.decode())
    bio_record = SeqRecord(
        seq,
        id=record.name.decode(),
        description="",
    )
    if record.quality:
        bio_record.letter_annotations["phred_quality"] = [b - 33 for b in record.quality]
    return bio_record


def main():
    # Create some BioPython records.
    bio_records = [
        SeqRecord(
            Seq("ACGTACGTACGTACGT"),
            id=f"read_{i}",
            description="",
            letter_annotations={"phred_quality": [30 + (i % 10)] * 16},
        )
        for i in range(50)
    ]

    print(f"Created {len(bio_records)} BioPython records")

    # Write them into a dryice buffer.
    writer = di.Writer.builder().target_block_records(10).build()
    for bio_record in bio_records:
        name, sequence, quality = bio_to_dryice(bio_record)
        writer.write_record(name, sequence, quality)
    data = writer.finish()

    print(f"Wrote {len(data)} bytes to dryice format")

    # Read them back and convert to BioPython.
    reader = di.Reader.open(data)
    recovered = [dryice_to_bio(record) for record in reader]

    print(f"Read back {len(recovered)} BioPython records")

    # Verify round-trip fidelity.
    for original, recovered_rec in zip(bio_records, recovered):
        assert original.id == recovered_rec.id
        assert str(original.seq) == str(recovered_rec.seq)
        assert (
            original.letter_annotations["phred_quality"]
            == recovered_rec.letter_annotations["phred_quality"]
        )

    print("All records match — round-trip verified")

    # Show a compact codec round-trip.
    writer = di.WriterBuilder().two_bit_exact().binned_quality().split_names().build()
    for bio_record in bio_records:
        name, sequence, quality = bio_to_dryice(bio_record)
        writer.write_record(name, sequence, quality)
    compact_data = writer.finish()

    print(
        f"Compact format: {len(compact_data)} bytes ({len(compact_data) * 100 // len(data)}% of raw)"
    )

    reader = di.ReaderBuilder().two_bit_exact().binned_quality().split_names().build(compact_data)
    compact_records = [dryice_to_bio(record) for record in reader]

    assert len(compact_records) == len(bio_records)
    for original, compact_rec in zip(bio_records, compact_records):
        assert str(original.seq) == str(compact_rec.seq)

    print("Compact round-trip verified (sequences match, quality is binned)")


if __name__ == "__main__":
    main()

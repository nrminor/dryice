import { test, expect, describe } from "bun:test";
import {
  WriterBuilder,
  Reader,
  ReaderBuilder,
  defaultMinimizerKey,
  defaultPrefixKmerKey,
} from "../api.js";

describe("Writer and Reader with default codecs", () => {
  test("round-trip two records", () => {
    const writer = new WriterBuilder().build();
    writer.writeRecord(
      Buffer.from("read1"),
      Buffer.from("ACGTACGT"),
      Buffer.from("!!!!!!!!")
    );
    writer.writeRecord(
      Buffer.from("read2"),
      Buffer.from("TGCATGCA"),
      Buffer.from("########")
    );
    const data = writer.finish();

    const reader = Reader.open(data);
    const records = reader.records();

    expect(records.length).toBe(2);
    expect(Buffer.from(records[0].name).toString()).toBe("read1");
    expect(Buffer.from(records[0].sequence).toString()).toBe("ACGTACGT");
    expect(Buffer.from(records[0].quality).toString()).toBe("!!!!!!!!");
    expect(Buffer.from(records[1].name).toString()).toBe("read2");
    expect(Buffer.from(records[1].sequence).toString()).toBe("TGCATGCA");
  });

  test("empty file", () => {
    const writer = new WriterBuilder().build();
    const data = writer.finish();

    const reader = Reader.open(data);
    const records = reader.records();

    expect(records.length).toBe(0);
  });

  test("many records across block boundaries", () => {
    const writer = new WriterBuilder().targetBlockRecords(10).build();
    for (let i = 0; i < 100; i++) {
      writer.writeRecord(
        Buffer.from(`read_${i}`),
        Buffer.from("ACGTACGT"),
        Buffer.from("!!!!!!!!")
      );
    }
    const data = writer.finish();

    const reader = Reader.open(data);
    const records = reader.records();

    expect(records.length).toBe(100);
    expect(Buffer.from(records[0].name).toString()).toBe("read_0");
    expect(Buffer.from(records[99].name).toString()).toBe("read_99");
  });
});

describe("Writer and Reader with TwoBitExact codec", () => {
  test("round-trip with ambiguity", () => {
    const writer = new WriterBuilder().twoBitExact().build();
    writer.writeRecord(
      Buffer.from("r1"),
      Buffer.from("ACNGT"),
      Buffer.from("!!!!!")
    );
    const data = writer.finish();

    const reader = new ReaderBuilder().twoBitExact().build(data);
    const records = reader.records();

    expect(records.length).toBe(1);
    expect(Buffer.from(records[0].sequence).toString()).toBe("ACNGT");
  });
});

describe("Writer and Reader with compact codecs", () => {
  test("round-trip with all compact codecs", () => {
    const writer = new WriterBuilder()
      .twoBitExact()
      .binnedQuality()
      .splitNames()
      .build();
    writer.writeRecord(
      Buffer.from("instrument:run:flowcell 1:N:0:ATCACG"),
      Buffer.from("ACGTACGT"),
      Buffer.from("!!!!!!!!")
    );
    const data = writer.finish();

    const reader = new ReaderBuilder()
      .twoBitExact()
      .binnedQuality()
      .splitNames()
      .build(data);
    const records = reader.records();

    expect(records.length).toBe(1);
    expect(Buffer.from(records[0].sequence).toString()).toBe("ACGTACGT");
    expect(records[0].quality.length).toBe(8);
  });

  test("selective decoding sequence only", () => {
    const writer = new WriterBuilder()
      .twoBitExact()
      .binnedQuality()
      .splitNames()
      .build();
    writer.writeRecord(
      Buffer.from("read1 desc"),
      Buffer.from("ACGTACGT"),
      Buffer.from("!!!!!!!!")
    );
    const data = writer.finish();

    const reader = new ReaderBuilder()
      .twoBitExact()
      .binnedQuality()
      .splitNames()
      .select("sequence")
      .build(data);
    const record = reader.nextRecord();

    expect(record).not.toBeNull();
    expect(Buffer.from(record!.sequence!).toString()).toBe("ACGTACGT");
    expect(record!.name).toBeUndefined();
    expect(record!.quality).toBeUndefined();
    expect(record!.key).toBeUndefined();
  });

  test("selective decoding quality only", () => {
    const writer = new WriterBuilder()
      .twoBitExact()
      .binnedQuality()
      .splitNames()
      .build();
    writer.writeRecord(
      Buffer.from("read1 desc"),
      Buffer.from("ACGTACGT"),
      Buffer.from("!!!!!!!!")
    );
    const data = writer.finish();

    const reader = new ReaderBuilder()
      .twoBitExact()
      .binnedQuality()
      .splitNames()
      .select("quality")
      .build(data);
    const record = reader.nextRecord();

    expect(record).not.toBeNull();
    expect(record!.quality).not.toBeNull();
    expect(record!.quality!.length).toBe(8);
    expect(record!.name).toBeUndefined();
    expect(record!.sequence).toBeUndefined();
    expect(record!.key).toBeUndefined();
  });

  test("selective decoding name only", () => {
    const writer = new WriterBuilder()
      .twoBitExact()
      .binnedQuality()
      .splitNames()
      .build();
    writer.writeRecord(
      Buffer.from("read1 desc"),
      Buffer.from("ACGTACGT"),
      Buffer.from("!!!!!!!!")
    );
    const data = writer.finish();

    const reader = new ReaderBuilder()
      .twoBitExact()
      .binnedQuality()
      .splitNames()
      .select("name")
      .build(data);
    const record = reader.nextRecord();

    expect(record).not.toBeNull();
    expect(Buffer.from(record!.name!).toString()).toBe("read1 desc");
    expect(record!.sequence).toBeUndefined();
    expect(record!.quality).toBeUndefined();
    expect(record!.key).toBeUndefined();
  });

  test("selective decoding sequence and key", () => {
    const writer = new WriterBuilder()
      .twoBitExact()
      .binnedQuality()
      .splitNames()
      .bytes8Key()
      .build();
    writer.writeRecordWithKey(
      Buffer.from("read1 desc"),
      Buffer.from("ACGTACGT"),
      Buffer.from("!!!!!!!!"),
      Buffer.from("sortkey!")
    );
    const data = writer.finish();

    const reader = new ReaderBuilder()
      .twoBitExact()
      .binnedQuality()
      .splitNames()
      .bytes8Key()
      .select("sequence", "key")
      .build(data);
    const record = reader.nextRecord();

    expect(record).not.toBeNull();
    expect(Buffer.from(record!.sequence!).toString()).toBe("ACGTACGT");
    expect(Buffer.from(record!.key!).toString()).toBe("sortkey!");
    expect(record!.name).toBeUndefined();
    expect(record!.quality).toBeUndefined();
  });

  test("selective decoding rejects unknown field", () => {
    expect(() => {
      new ReaderBuilder().select("banana");
    }).toThrow();
  });

  test("variadic select returns records with omitted properties absent", () => {
    const writer = new WriterBuilder()
      .twoBitExact()
      .binnedQuality()
      .splitNames()
      .build();
    writer.writeRecord(
      Buffer.from("read1 desc"),
      Buffer.from("ACGTACGT"),
      Buffer.from("!!!!!!!!")
    );
    const data = writer.finish();

    const reader = new ReaderBuilder()
      .twoBitExact()
      .binnedQuality()
      .splitNames()
      .select("sequence")
      .build(data);
    const record = reader.nextRecord();

    expect(record).not.toBeNull();
    expect("sequence" in record!).toBe(true);
    expect("name" in record!).toBe(false);
    expect("quality" in record!).toBe(false);
    expect("key" in record!).toBe(false);
  });
});

describe("Writer and Reader with record keys", () => {
  test("round-trip with bytes8 key", () => {
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

    expect(records.length).toBe(1);
    expect(records[0].key).not.toBeNull();
    expect(Buffer.from(records[0].key!).toString()).toBe("sortkey!");
  });

  test("default prefix kmer key helper returns 8-byte key", () => {
    const key = defaultPrefixKmerKey(Buffer.from("ACGTACGTACGTACGTACGTACGTACGTACG"));
    expect(key).not.toBeNull();
    expect(key!.length).toBe(8);
  });

  test("default minimizer key helper returns 8-byte key", () => {
    const key = defaultMinimizerKey(
      Buffer.from("ACGTGCTCAGAGACTCAGAGGATTACAGTTTACGTGCTCAGAGACTCAGAGGA")
    );
    expect(key).not.toBeNull();
    expect(key!.length).toBe(8);
  });

  test("minimizers preset round-trips key-only payload", () => {
    const writer = new WriterBuilder().minimizers().build();
    const key = defaultMinimizerKey(
      Buffer.from("ACGTGCTCAGAGACTCAGAGGATTACAGTTTACGTGCTCAGAGACTCAGAGGA")
    );
    expect(key).not.toBeNull();
    writer.writeRecordWithKey(Buffer.from(""), Buffer.from(""), Buffer.from(""), key!);
    const data = writer.finish();

    const reader = new ReaderBuilder().minimizers().build(data);
    const record = reader.nextRecord();

    expect(record).not.toBeNull();
    expect(record!.key).toEqual(key);
    expect(Buffer.from(record!.name!).length).toBe(0);
    expect(Buffer.from(record!.sequence!).length).toBe(0);
    expect(Buffer.from(record!.quality!).length).toBe(0);
  });

  test("prefix kmers with sequences preset keeps sequence and key", () => {
    const sequence = Buffer.from("ACGTACGTACGTACGTACGTACGTACGTACG");
    const key = defaultPrefixKmerKey(sequence);
    expect(key).not.toBeNull();

    const writer = new WriterBuilder().prefixKmersWithSequences().build();
    writer.writeRecordWithKey(Buffer.from(""), sequence, Buffer.from("!".repeat(sequence.length)), key!);
    const data = writer.finish();

    const reader = new ReaderBuilder().prefixKmersWithSequences().build(data);
    const record = reader.nextRecord();

    expect(record).not.toBeNull();
    expect(record!.key).toEqual(key);
    expect(Buffer.from(record!.sequence!).toString()).toBe(sequence.toString());
    expect(Buffer.from(record!.name!).length).toBe(0);
    expect(Buffer.from(record!.quality!).length).toBe(0);
  });
});

describe("Writer error handling", () => {
  test("rejects write after finish", () => {
    const writer = new WriterBuilder().build();
    writer.finish();

    expect(() => {
      writer.writeRecord(
        Buffer.from("r1"),
        Buffer.from("ACGT"),
        Buffer.from("!!!!")
      );
    }).toThrow();
  });

  test("rejects double finish", () => {
    const writer = new WriterBuilder().build();
    writer.finish();

    expect(() => {
      writer.finish();
    }).toThrow();
  });
});

describe("Reader iteration", () => {
  test("nextRecord returns records then null", () => {
    const writer = new WriterBuilder().build();
    writer.writeRecord(
      Buffer.from("r1"),
      Buffer.from("ACGT"),
      Buffer.from("!!!!")
    );
    const data = writer.finish();

    const reader = Reader.open(data);
    const first = reader.nextRecord();
    expect(first).not.toBeNull();
    expect(Buffer.from(first!.name).toString()).toBe("r1");

    const second = reader.nextRecord();
    expect(second).toBeNull();
  });
});

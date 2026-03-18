import { test, expect, describe } from "bun:test";
import { WriterBuilder, Reader, ReaderBuilder } from "../index.js";

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

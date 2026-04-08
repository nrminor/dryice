const native = require('./index.js')

class Reader {
  #inner

  constructor(inner) {
    this.#inner = inner
  }

  static open(data) {
    return new Reader(native.Reader.open(data))
  }

  nextRecord() {
    return this.#inner.nextRecord()
  }

  records() {
    return this.#inner.records()
  }
}

class WriterBuilder {
  #inner

  constructor() {
    this.#inner = new native.WriterBuilder()
  }

  twoBitExact() {
    this.#inner.twoBitExact()
    return this
  }

  twoBitLossyN() {
    this.#inner.twoBitLossyN()
    return this
  }

  binnedQuality() {
    this.#inner.binnedQuality()
    return this
  }

  splitNames() {
    this.#inner.splitNames()
    return this
  }

  bytes8Key() {
    this.#inner.bytes8Key()
    return this
  }

  prefixKmers() {
    this.#inner.prefixKmers()
    return this
  }

  prefixKmersWithSequences() {
    this.#inner.prefixKmersWithSequences()
    return this
  }

  prefixKmersWithNames() {
    this.#inner.prefixKmersWithNames()
    return this
  }

  minimizers() {
    this.#inner.minimizers()
    return this
  }

  minimizersWithSequences() {
    this.#inner.minimizersWithSequences()
    return this
  }

  minimizersWithNames() {
    this.#inner.minimizersWithNames()
    return this
  }

  targetBlockRecords(n) {
    this.#inner.targetBlockRecords(n)
    return this
  }

  build() {
    return new Writer(this.#inner.build())
  }
}

class Writer {
  #inner

  constructor(inner) {
    this.#inner = inner
  }

  writeRecord(name, sequence, quality) {
    return this.#inner.writeRecord(name, sequence, quality)
  }

  writeRecordWithKey(name, sequence, quality, key) {
    return this.#inner.writeRecordWithKey(name, sequence, quality, key)
  }

  finish() {
    return this.#inner.finish()
  }
}

class ReaderBuilder {
  #inner

  constructor() {
    this.#inner = new native.ReaderBuilder()
  }

  twoBitExact() {
    this.#inner.twoBitExact()
    return this
  }

  twoBitLossyN() {
    this.#inner.twoBitLossyN()
    return this
  }

  binnedQuality() {
    this.#inner.binnedQuality()
    return this
  }

  splitNames() {
    this.#inner.splitNames()
    return this
  }

  bytes8Key() {
    this.#inner.bytes8Key()
    return this
  }

  prefixKmers() {
    this.#inner.prefixKmers()
    return this
  }

  prefixKmersWithSequences() {
    this.#inner.prefixKmersWithSequences()
    return this
  }

  prefixKmersWithNames() {
    this.#inner.prefixKmersWithNames()
    return this
  }

  minimizers() {
    this.#inner.minimizers()
    return this
  }

  minimizersWithSequences() {
    this.#inner.minimizersWithSequences()
    return this
  }

  minimizersWithNames() {
    this.#inner.minimizersWithNames()
    return this
  }

  select(...fields) {
    this.#inner.select(fields)
    return this
  }

  build(data) {
    return new Reader(this.#inner.build(data))
  }
}

module.exports = {
  defaultPrefixKmerKey: native.defaultPrefixKmerKey,
  defaultMinimizerKey: native.defaultMinimizerKey,
  Reader,
  ReaderBuilder,
  Writer,
  WriterBuilder,
}

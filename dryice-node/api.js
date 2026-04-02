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

  select(...fields) {
    this.#inner.select(fields)
    return this
  }

  build(data) {
    return new Reader(this.#inner.build(data))
  }
}

module.exports = {
  Reader,
  ReaderBuilder,
  Writer: native.Writer,
  WriterBuilder: native.WriterBuilder,
}

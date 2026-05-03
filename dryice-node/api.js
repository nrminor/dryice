const native = require('./index.js')

const nativeTempFile = Symbol('nativeTempFile')

function warnCleanupFailure(error) {
  const message = error instanceof Error ? error.message : String(error)
  process.emitWarning(`failed to clean up temporary dryice file: ${message}`)
}

function cleanupTempFile(tempFile) {
  try {
    tempFile.cleanup()
  } catch (error) {
    warnCleanupFailure(error)
  }
}

function isPromiseLike(value) {
  return (
    value !== null &&
    (typeof value === 'object' || typeof value === 'function') &&
    typeof value.then === 'function'
  )
}

class TempFile {
  #inner

  constructor(inner = new native.TempFile()) {
    this.#inner = inner
  }

  get path() {
    return this.#inner.path
  }

  cleanup() {
    return this.#inner.cleanup()
  }

  persist(path) {
    return this.#inner.persist(path)
  }

  [nativeTempFile]() {
    return this.#inner
  }
}

function tempFile() {
  return new TempFile(native.tempFile())
}

function withTempFile(callback) {
  const tmp = tempFile()
  try {
    const result = callback(tmp)
    if (isPromiseLike(result)) {
      return Promise.resolve(result).finally(() => cleanupTempFile(tmp))
    }
    cleanupTempFile(tmp)
    return result
  } catch (error) {
    cleanupTempFile(tmp)
    throw error
  }
}

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

  buildTemp(tempFile) {
    return new TempWriter(this.#inner.buildTemp(tempFile[nativeTempFile]()))
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

class TempWriter {
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

  buildTemp(tempFile) {
    return new Reader(this.#inner.buildTemp(tempFile[nativeTempFile]()))
  }
}

module.exports = {
  defaultPrefixKmerKey: native.defaultPrefixKmerKey,
  defaultMinimizerKey: native.defaultMinimizerKey,
  Reader,
  ReaderBuilder,
  TempFile,
  TempWriter,
  Writer,
  WriterBuilder,
  tempFile,
  withTempFile,
}

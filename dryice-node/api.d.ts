export type Field = 'name' | 'sequence' | 'quality' | 'key'

export type FullRecord = {
  name: Buffer
  sequence: Buffer
  quality: Buffer
  key?: Buffer
}

type HasField<F extends readonly Field[], T extends Field> = T extends F[number]
  ? true
  : false

export type ProjectedRecord<F extends readonly Field[]> =
  (HasField<F, 'name'> extends true ? { name: Buffer } : {}) &
  (HasField<F, 'sequence'> extends true ? { sequence: Buffer } : {}) &
  (HasField<F, 'quality'> extends true ? { quality: Buffer } : {}) &
  (HasField<F, 'key'> extends true ? { key: Buffer } : {})

export declare class Reader<F extends readonly Field[] | null = null> {
  static open(data: Buffer): Reader<null>
  nextRecord(): (F extends readonly Field[] ? ProjectedRecord<F> : FullRecord) | null
  records(): Array<F extends readonly Field[] ? ProjectedRecord<F> : FullRecord>
}

export declare class ReaderBuilder<F extends readonly Field[] | null = null> {
  constructor()
  twoBitExact(): ReaderBuilder<F>
  twoBitLossyN(): ReaderBuilder<F>
  binnedQuality(): ReaderBuilder<F>
  splitNames(): ReaderBuilder<F>
  bytes8Key(): ReaderBuilder<F>
  prefixKmers(): ReaderBuilder<F>
  prefixKmersWithSequences(): ReaderBuilder<F>
  prefixKmersWithNames(): ReaderBuilder<F>
  minimizers(): ReaderBuilder<F>
  minimizersWithSequences(): ReaderBuilder<F>
  minimizersWithNames(): ReaderBuilder<F>
  select<const G extends readonly Field[]>(...fields: G): ReaderBuilder<G>
  build(data: Buffer): Reader<F>
  buildTemp(tempFile: TempFile): Reader<F>
}

export declare class Writer {
  writeRecord(name: Buffer, sequence: Buffer, quality: Buffer): void
  writeRecordWithKey(name: Buffer, sequence: Buffer, quality: Buffer, key: Buffer): void
  finish(): Buffer
}

export declare class WriterBuilder {
  constructor()
  twoBitExact(): this
  twoBitLossyN(): this
  binnedQuality(): this
  splitNames(): this
  bytes8Key(): this
  prefixKmers(): this
  prefixKmersWithSequences(): this
  prefixKmersWithNames(): this
  minimizers(): this
  minimizersWithSequences(): this
  minimizersWithNames(): this
  targetBlockRecords(n: number): this
  build(): Writer
  buildTemp(tempFile: TempFile): TempWriter
}

export declare class TempFile {
  constructor()
  readonly path: string
  cleanup(): void
  persist(path: string): string
}

export declare class TempWriter {
  writeRecord(name: Buffer, sequence: Buffer, quality: Buffer): void
  writeRecordWithKey(name: Buffer, sequence: Buffer, quality: Buffer, key: Buffer): void
  finish(): void
}

export declare function defaultPrefixKmerKey(sequence: Buffer): Buffer | null
export declare function defaultMinimizerKey(sequence: Buffer): Buffer | null
export declare function tempFile(): TempFile
export declare function withTempFile<T>(callback: (tempFile: TempFile) => T): T

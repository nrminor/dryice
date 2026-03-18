//! Python bindings for the `dryice` high-throughput genomic record container.

use pyo3::prelude::*;

use dryice::{
    BinnedQualityCodec, RawAsciiCodec, RawNameCodec, RawQualityCodec, SplitNameCodec,
    TwoBitExactCodec, TwoBitLossyNCodec,
};
use dryice::{
    Bytes8Key, DryIceError, DryIceReader as RustReader, DryIceWriter as RustWriter, NoRecordKey,
    SeqRecordLike,
};

fn to_py_err(e: DryIceError) -> PyErr {
    PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string())
}

struct SliceRecord<'a> {
    name: &'a [u8],
    sequence: &'a [u8],
    quality: &'a [u8],
}

impl SeqRecordLike for SliceRecord<'_> {
    fn name(&self) -> &[u8] {
        self.name
    }
    fn sequence(&self) -> &[u8] {
        self.sequence
    }
    fn quality(&self) -> &[u8] {
        self.quality
    }
}

type W = Vec<u8>;

macro_rules! dispatch_all_writers {
    ($self:expr, $method:ident ( $($arg:expr),* )) => {
        match $self {
            WriterInner::RawRawRaw(w) => w.$method($($arg),*),
            WriterInner::TwoBitRawRaw(w) => w.$method($($arg),*),
            WriterInner::TwoBitBinnedSplit(w) => w.$method($($arg),*),
            WriterInner::LossyBinnedSplit(w) => w.$method($($arg),*),
            WriterInner::RawRawRawB8(w) => w.$method($($arg),*),
            WriterInner::TwoBitBinnedSplitB8(w) => w.$method($($arg),*),
        }
    };
}

macro_rules! dispatch_all_readers {
    ($self:expr, $method:ident ( $($arg:expr),* )) => {
        match $self {
            ReaderInner::RawRawRaw(r) => r.$method($($arg),*),
            ReaderInner::TwoBitRawRaw(r) => r.$method($($arg),*),
            ReaderInner::TwoBitBinnedSplit(r) => r.$method($($arg),*),
            ReaderInner::LossyBinnedSplit(r) => r.$method($($arg),*),
            ReaderInner::RawRawRawB8(r) => r.$method($($arg),*),
        }
    };
}

enum WriterInner {
    RawRawRaw(RustWriter<W, RawAsciiCodec, RawQualityCodec, RawNameCodec, NoRecordKey>),
    TwoBitRawRaw(RustWriter<W, TwoBitExactCodec, RawQualityCodec, RawNameCodec, NoRecordKey>),
    TwoBitBinnedSplit(
        RustWriter<W, TwoBitExactCodec, BinnedQualityCodec, SplitNameCodec, NoRecordKey>,
    ),
    LossyBinnedSplit(
        RustWriter<W, TwoBitLossyNCodec, BinnedQualityCodec, SplitNameCodec, NoRecordKey>,
    ),
    RawRawRawB8(RustWriter<W, RawAsciiCodec, RawQualityCodec, RawNameCodec, Bytes8Key>),
    TwoBitBinnedSplitB8(
        RustWriter<W, TwoBitExactCodec, BinnedQualityCodec, SplitNameCodec, Bytes8Key>,
    ),
}

impl WriterInner {
    fn write_record(&mut self, record: &SliceRecord<'_>) -> Result<(), DryIceError> {
        match self {
            Self::RawRawRaw(w) => w.write_record(record),
            Self::TwoBitRawRaw(w) => w.write_record(record),
            Self::TwoBitBinnedSplit(w) => w.write_record(record),
            Self::LossyBinnedSplit(w) => w.write_record(record),
            Self::RawRawRawB8(_) | Self::TwoBitBinnedSplitB8(_) => {
                Err(DryIceError::InvalidWriterConfiguration(
                    "use write_record_with_key for keyed writers",
                ))
            },
        }
    }

    fn write_record_with_key(
        &mut self,
        record: &SliceRecord<'_>,
        key: &[u8],
    ) -> Result<(), DryIceError> {
        match self {
            Self::RawRawRawB8(w) => {
                let k = Bytes8Key(key.try_into().map_err(|_| {
                    DryIceError::InvalidRecordKeyEncoding {
                        message: "key must be exactly 8 bytes",
                    }
                })?);
                w.write_record_with_key(record, &k)
            },
            Self::TwoBitBinnedSplitB8(w) => {
                let k = Bytes8Key(key.try_into().map_err(|_| {
                    DryIceError::InvalidRecordKeyEncoding {
                        message: "key must be exactly 8 bytes",
                    }
                })?);
                w.write_record_with_key(record, &k)
            },
            _ => Err(DryIceError::InvalidWriterConfiguration(
                "write_record_with_key requires a keyed writer",
            )),
        }
    }

    fn finish(self) -> Result<Vec<u8>, DryIceError> {
        dispatch_all_writers!(self, finish())
    }
}

type R = std::io::Cursor<Vec<u8>>;

enum ReaderInner {
    RawRawRaw(RustReader<R, RawAsciiCodec, RawQualityCodec, RawNameCodec, NoRecordKey>),
    TwoBitRawRaw(RustReader<R, TwoBitExactCodec, RawQualityCodec, RawNameCodec, NoRecordKey>),
    TwoBitBinnedSplit(
        RustReader<R, TwoBitExactCodec, BinnedQualityCodec, SplitNameCodec, NoRecordKey>,
    ),
    LossyBinnedSplit(
        RustReader<R, TwoBitLossyNCodec, BinnedQualityCodec, SplitNameCodec, NoRecordKey>,
    ),
    RawRawRawB8(RustReader<R, RawAsciiCodec, RawQualityCodec, RawNameCodec, Bytes8Key>),
}

impl ReaderInner {
    fn next_record(&mut self) -> Result<bool, DryIceError> {
        dispatch_all_readers!(self, next_record())
    }

    fn name(&self) -> &[u8] {
        dispatch_all_readers!(self, name())
    }

    fn sequence(&self) -> &[u8] {
        dispatch_all_readers!(self, sequence())
    }

    fn quality(&self) -> &[u8] {
        dispatch_all_readers!(self, quality())
    }

    fn record_key(&self) -> Result<Option<Vec<u8>>, DryIceError> {
        match self {
            Self::RawRawRawB8(r) => {
                let k = r.record_key()?;
                Ok(Some(k.0.to_vec()))
            },
            _ => Ok(None),
        }
    }
}

#[pymodule]
mod dryice_python {
    use super::*;

    /// A builder for configuring a DryIce writer.
    #[pyclass]
    struct WriterBuilder {
        sequence_codec: String,
        quality_codec: String,
        name_codec: String,
        record_key: String,
        target_block_records: usize,
    }

    #[pymethods]
    impl WriterBuilder {
        #[new]
        fn new() -> Self {
            Self {
                sequence_codec: "raw".to_string(),
                quality_codec: "raw".to_string(),
                name_codec: "raw".to_string(),
                record_key: "none".to_string(),
                target_block_records: 8192,
            }
        }

        fn two_bit_exact(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
            slf.sequence_codec = "two_bit_exact".to_string();
            slf
        }

        fn two_bit_lossy_n(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
            slf.sequence_codec = "two_bit_lossy_n".to_string();
            slf
        }

        fn binned_quality(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
            slf.quality_codec = "binned".to_string();
            slf
        }

        fn split_names(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
            slf.name_codec = "split".to_string();
            slf
        }

        fn bytes8_key(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
            slf.record_key = "bytes8".to_string();
            slf
        }

        fn target_block_records(mut slf: PyRefMut<'_, Self>, n: usize) -> PyRefMut<'_, Self> {
            slf.target_block_records = n;
            slf
        }

        fn build(&self) -> PyResult<Writer> {
            let n = self.target_block_records;

            let inner = match (
                self.sequence_codec.as_str(),
                self.quality_codec.as_str(),
                self.name_codec.as_str(),
                self.record_key.as_str(),
            ) {
                ("raw", "raw", "raw", "none") => WriterInner::RawRawRaw(
                    RustWriter::builder()
                        .inner(Vec::new())
                        .target_block_records(n)
                        .build(),
                ),
                ("two_bit_exact", "raw", "raw", "none") => WriterInner::TwoBitRawRaw(
                    RustWriter::builder()
                        .inner(Vec::new())
                        .two_bit_exact()
                        .target_block_records(n)
                        .build(),
                ),
                ("two_bit_exact", "binned", "split", "none") => WriterInner::TwoBitBinnedSplit(
                    RustWriter::builder()
                        .inner(Vec::new())
                        .two_bit_exact()
                        .binned_quality()
                        .split_names()
                        .target_block_records(n)
                        .build(),
                ),
                ("two_bit_lossy_n", "binned", "split", "none") => WriterInner::LossyBinnedSplit(
                    RustWriter::builder()
                        .inner(Vec::new())
                        .two_bit_lossy_n()
                        .binned_quality()
                        .split_names()
                        .target_block_records(n)
                        .build(),
                ),
                ("raw", "raw", "raw", "bytes8") => WriterInner::RawRawRawB8(
                    RustWriter::builder()
                        .inner(Vec::new())
                        .bytes8_key()
                        .target_block_records(n)
                        .build(),
                ),
                ("two_bit_exact", "binned", "split", "bytes8") => WriterInner::TwoBitBinnedSplitB8(
                    RustWriter::builder()
                        .inner(Vec::new())
                        .two_bit_exact()
                        .binned_quality()
                        .split_names()
                        .bytes8_key()
                        .target_block_records(n)
                        .build(),
                ),
                _ => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                        "unsupported codec combination: seq={}, qual={}, name={}, key={}",
                        self.sequence_codec, self.quality_codec, self.name_codec, self.record_key
                    )));
                },
            };

            Ok(Writer { inner: Some(inner) })
        }
    }

    /// A dryice file writer.
    #[pyclass]
    struct Writer {
        inner: Option<WriterInner>,
    }

    #[pymethods]
    impl Writer {
        /// Create a new writer builder.
        #[staticmethod]
        fn builder() -> WriterBuilder {
            WriterBuilder::new()
        }

        /// Write a single record.
        fn write_record(&mut self, name: &[u8], sequence: &[u8], quality: &[u8]) -> PyResult<()> {
            let record = SliceRecord {
                name,
                sequence,
                quality,
            };
            self.inner
                .as_mut()
                .ok_or_else(|| {
                    PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("writer already finished")
                })?
                .write_record(&record)
                .map_err(to_py_err)
        }

        /// Write a single record with an accelerator key.
        fn write_record_with_key(
            &mut self,
            name: &[u8],
            sequence: &[u8],
            quality: &[u8],
            key: &[u8],
        ) -> PyResult<()> {
            let record = SliceRecord {
                name,
                sequence,
                quality,
            };
            self.inner
                .as_mut()
                .ok_or_else(|| {
                    PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("writer already finished")
                })?
                .write_record_with_key(&record, key)
                .map_err(to_py_err)
        }

        /// Finish writing and return the serialized bytes.
        fn finish(&mut self) -> PyResult<Vec<u8>> {
            self.inner
                .take()
                .ok_or_else(|| {
                    PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("writer already finished")
                })?
                .finish()
                .map_err(to_py_err)
        }
    }

    /// A decoded sequencing record.
    #[pyclass]
    #[derive(Clone)]
    struct Record {
        #[pyo3(get)]
        name: Vec<u8>,
        #[pyo3(get)]
        sequence: Vec<u8>,
        #[pyo3(get)]
        quality: Vec<u8>,
        #[pyo3(get)]
        key: Option<Vec<u8>>,
    }

    #[pymethods]
    impl Record {
        fn __repr__(&self) -> String {
            let name = String::from_utf8_lossy(&self.name);
            format!("Record(name={name}, len={})", self.sequence.len())
        }
    }

    /// A builder for configuring a DryIce reader.
    #[pyclass]
    struct ReaderBuilder {
        sequence_codec: String,
        quality_codec: String,
        name_codec: String,
        record_key: String,
    }

    #[pymethods]
    impl ReaderBuilder {
        #[new]
        fn new() -> Self {
            Self {
                sequence_codec: "raw".to_string(),
                quality_codec: "raw".to_string(),
                name_codec: "raw".to_string(),
                record_key: "none".to_string(),
            }
        }

        fn two_bit_exact(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
            slf.sequence_codec = "two_bit_exact".to_string();
            slf
        }

        fn two_bit_lossy_n(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
            slf.sequence_codec = "two_bit_lossy_n".to_string();
            slf
        }

        fn binned_quality(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
            slf.quality_codec = "binned".to_string();
            slf
        }

        fn split_names(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
            slf.name_codec = "split".to_string();
            slf
        }

        fn bytes8_key(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
            slf.record_key = "bytes8".to_string();
            slf
        }

        fn build(&self, data: Vec<u8>) -> PyResult<Reader> {
            let cursor = std::io::Cursor::new(data);

            let codec_key = (
                self.sequence_codec.as_str(),
                self.quality_codec.as_str(),
                self.name_codec.as_str(),
                self.record_key.as_str(),
            );

            let inner = match codec_key {
                ("raw", "raw", "raw", "none") => {
                    ReaderInner::RawRawRaw(RustReader::new(cursor).map_err(to_py_err)?)
                },
                ("two_bit_exact", "raw", "raw", "none") => ReaderInner::TwoBitRawRaw(
                    RustReader::with_two_bit_exact(cursor).map_err(to_py_err)?,
                ),
                ("two_bit_exact", "binned", "split", "none") => {
                    ReaderInner::TwoBitBinnedSplit(
                        RustReader::with_codecs::<
                            TwoBitExactCodec,
                            BinnedQualityCodec,
                            SplitNameCodec,
                        >(cursor)
                        .map_err(to_py_err)?,
                    )
                },
                ("two_bit_lossy_n", "binned", "split", "none") => {
                    ReaderInner::LossyBinnedSplit(
                        RustReader::with_codecs::<
                            TwoBitLossyNCodec,
                            BinnedQualityCodec,
                            SplitNameCodec,
                        >(cursor)
                        .map_err(to_py_err)?,
                    )
                },
                ("raw", "raw", "raw", "bytes8") => ReaderInner::RawRawRawB8(
                    RustReader::with_bytes8_key(cursor).map_err(to_py_err)?,
                ),
                ("two_bit_exact", "binned", "split", "bytes8") => {
                    // Need a combined constructor — for now, use the raw path
                    // and rely on codec tag verification at block load time.
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "combined codec + key reader not yet supported from Python",
                    ));
                },
                _ => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                        "unsupported codec combination: seq={}, qual={}, name={}, key={}",
                        self.sequence_codec, self.quality_codec, self.name_codec, self.record_key
                    )));
                },
            };

            Ok(Reader { inner })
        }
    }

    /// A dryice file reader.
    #[pyclass]
    struct Reader {
        inner: ReaderInner,
    }

    #[pymethods]
    impl Reader {
        /// Create a reader builder.
        #[staticmethod]
        fn builder() -> ReaderBuilder {
            ReaderBuilder::new()
        }

        /// Open a dryice file with default codecs.
        #[staticmethod]
        fn open(data: Vec<u8>) -> PyResult<Self> {
            let cursor = std::io::Cursor::new(data);
            let inner = ReaderInner::RawRawRaw(RustReader::new(cursor).map_err(to_py_err)?);
            Ok(Self { inner })
        }

        fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
            slf
        }

        fn __next__(&mut self) -> PyResult<Option<Record>> {
            let has_record = self.inner.next_record().map_err(to_py_err)?;
            if !has_record {
                return Ok(None);
            }

            let name = self.inner.name().to_vec();
            let sequence = self.inner.sequence().to_vec();
            let quality = self.inner.quality().to_vec();
            let key = self.inner.record_key().map_err(to_py_err)?;

            Ok(Some(Record {
                name,
                sequence,
                quality,
                key,
            }))
        }
    }
}

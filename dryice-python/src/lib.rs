//! Python bindings for the `dryice` high-throughput genomic record container.

use pyo3::prelude::*;
use pyo3::wrap_pyfunction;

use dryice::{
    BinnedQualityCodec, Bytes8Key, DryIceError, DryIceReader as RustReader,
    DryIceWriter as RustWriter, NoRecordKey, SelectedDryIceReader as RustSelectedReader,
    SeqRecordLike, SplitNameCodec, TwoBitExactCodec, TwoBitLossyNCodec,
    fields::{Key as SelectKey, Name as SelectName, Quality as SelectQuality},
    fields::{Sequence as SelectSequence, SequenceKey as SelectSequenceKey},
};
use dryice::{RawAsciiCodec, RawNameCodec, RawQualityCodec};

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
type R = std::io::Cursor<Vec<u8>>;

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
            ReaderInner::TwoBitBinnedSplitB8(r) => r.$method($($arg),*),
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

#[derive(Clone, Copy)]
enum Projection {
    All,
    Name,
    Sequence,
    Quality,
    Key,
    SequenceKey,
}

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
    TwoBitBinnedSplitB8(
        RustReader<R, TwoBitExactCodec, BinnedQualityCodec, SplitNameCodec, Bytes8Key>,
    ),
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
            Self::RawRawRawB8(r) => Ok(Some(r.record_key()?.0.to_vec())),
            Self::TwoBitBinnedSplitB8(r) => Ok(Some(r.record_key()?.0.to_vec())),
            _ => Ok(None),
        }
    }
}

enum SelectedReaderInner {
    RawRawRawName(
        RustSelectedReader<
            R,
            RawAsciiCodec,
            RawQualityCodec,
            RawNameCodec,
            NoRecordKey,
            SelectName,
        >,
    ),
    RawRawRawSequence(
        RustSelectedReader<
            R,
            RawAsciiCodec,
            RawQualityCodec,
            RawNameCodec,
            NoRecordKey,
            SelectSequence,
        >,
    ),
    RawRawRawQuality(
        RustSelectedReader<
            R,
            RawAsciiCodec,
            RawQualityCodec,
            RawNameCodec,
            NoRecordKey,
            SelectQuality,
        >,
    ),
    TwoBitRawRawName(
        RustSelectedReader<
            R,
            TwoBitExactCodec,
            RawQualityCodec,
            RawNameCodec,
            NoRecordKey,
            SelectName,
        >,
    ),
    TwoBitRawRawSequence(
        RustSelectedReader<
            R,
            TwoBitExactCodec,
            RawQualityCodec,
            RawNameCodec,
            NoRecordKey,
            SelectSequence,
        >,
    ),
    TwoBitRawRawQuality(
        RustSelectedReader<
            R,
            TwoBitExactCodec,
            RawQualityCodec,
            RawNameCodec,
            NoRecordKey,
            SelectQuality,
        >,
    ),
    TwoBitBinnedSplitName(
        RustSelectedReader<
            R,
            TwoBitExactCodec,
            BinnedQualityCodec,
            SplitNameCodec,
            NoRecordKey,
            SelectName,
        >,
    ),
    TwoBitBinnedSplitSequence(
        RustSelectedReader<
            R,
            TwoBitExactCodec,
            BinnedQualityCodec,
            SplitNameCodec,
            NoRecordKey,
            SelectSequence,
        >,
    ),
    TwoBitBinnedSplitQuality(
        RustSelectedReader<
            R,
            TwoBitExactCodec,
            BinnedQualityCodec,
            SplitNameCodec,
            NoRecordKey,
            SelectQuality,
        >,
    ),
    LossyBinnedSplitName(
        RustSelectedReader<
            R,
            TwoBitLossyNCodec,
            BinnedQualityCodec,
            SplitNameCodec,
            NoRecordKey,
            SelectName,
        >,
    ),
    LossyBinnedSplitSequence(
        RustSelectedReader<
            R,
            TwoBitLossyNCodec,
            BinnedQualityCodec,
            SplitNameCodec,
            NoRecordKey,
            SelectSequence,
        >,
    ),
    LossyBinnedSplitQuality(
        RustSelectedReader<
            R,
            TwoBitLossyNCodec,
            BinnedQualityCodec,
            SplitNameCodec,
            NoRecordKey,
            SelectQuality,
        >,
    ),
    RawRawRawB8Key(
        RustSelectedReader<R, RawAsciiCodec, RawQualityCodec, RawNameCodec, Bytes8Key, SelectKey>,
    ),
    RawRawRawB8SequenceKey(
        RustSelectedReader<
            R,
            RawAsciiCodec,
            RawQualityCodec,
            RawNameCodec,
            Bytes8Key,
            SelectSequenceKey,
        >,
    ),
    TwoBitBinnedSplitB8Key(
        RustSelectedReader<
            R,
            TwoBitExactCodec,
            BinnedQualityCodec,
            SplitNameCodec,
            Bytes8Key,
            SelectKey,
        >,
    ),
    TwoBitBinnedSplitB8SequenceKey(
        RustSelectedReader<
            R,
            TwoBitExactCodec,
            BinnedQualityCodec,
            SplitNameCodec,
            Bytes8Key,
            SelectSequenceKey,
        >,
    ),
}

enum ProjectedRecordData {
    Name(Vec<u8>),
    Sequence(Vec<u8>),
    Quality(Vec<u8>),
    Key(Vec<u8>),
    SequenceKey { sequence: Vec<u8>, key: Vec<u8> },
}

fn next_name_record<S, Q, N, K>(
    reader: &mut RustSelectedReader<R, S, Q, N, K, SelectName>,
) -> Result<Option<ProjectedRecordData>, DryIceError>
where
    S: dryice::SequenceCodec,
    Q: dryice::QualityCodec,
    N: dryice::NameCodec,
{
    Ok(reader
        .next_record()?
        .map(|record| ProjectedRecordData::Name(record.name().to_vec())))
}

fn next_sequence_record<S, Q, N, K>(
    reader: &mut RustSelectedReader<R, S, Q, N, K, SelectSequence>,
) -> Result<Option<ProjectedRecordData>, DryIceError>
where
    S: dryice::SequenceCodec,
    Q: dryice::QualityCodec,
    N: dryice::NameCodec,
{
    Ok(reader
        .next_record()?
        .map(|record| ProjectedRecordData::Sequence(record.sequence().to_vec())))
}

fn next_quality_record<S, Q, N, K>(
    reader: &mut RustSelectedReader<R, S, Q, N, K, SelectQuality>,
) -> Result<Option<ProjectedRecordData>, DryIceError>
where
    S: dryice::SequenceCodec,
    Q: dryice::QualityCodec,
    N: dryice::NameCodec,
{
    Ok(reader
        .next_record()?
        .map(|record| ProjectedRecordData::Quality(record.quality().to_vec())))
}

fn next_key_record<S, Q, N>(
    reader: &mut RustSelectedReader<R, S, Q, N, Bytes8Key, SelectKey>,
) -> Result<Option<ProjectedRecordData>, DryIceError>
where
    S: dryice::SequenceCodec,
    Q: dryice::QualityCodec,
    N: dryice::NameCodec,
{
    if let Some(record) = reader.next_record()? {
        Ok(Some(ProjectedRecordData::Key(
            record.record_key()?.0.to_vec(),
        )))
    } else {
        Ok(None)
    }
}

fn next_sequence_key_record<S, Q, N>(
    reader: &mut RustSelectedReader<R, S, Q, N, Bytes8Key, SelectSequenceKey>,
) -> Result<Option<ProjectedRecordData>, DryIceError>
where
    S: dryice::SequenceCodec,
    Q: dryice::QualityCodec,
    N: dryice::NameCodec,
{
    if let Some(record) = reader.next_record()? {
        Ok(Some(ProjectedRecordData::SequenceKey {
            sequence: record.sequence().to_vec(),
            key: record.record_key()?.0.to_vec(),
        }))
    } else {
        Ok(None)
    }
}

impl SelectedReaderInner {
    fn next_projected_record(&mut self) -> Result<Option<ProjectedRecordData>, DryIceError> {
        match self {
            Self::RawRawRawName(r) => next_name_record(r),
            Self::TwoBitRawRawName(r) => next_name_record(r),
            Self::TwoBitBinnedSplitName(r) => next_name_record(r),
            Self::LossyBinnedSplitName(r) => next_name_record(r),
            Self::RawRawRawSequence(r) => next_sequence_record(r),
            Self::TwoBitRawRawSequence(r) => next_sequence_record(r),
            Self::TwoBitBinnedSplitSequence(r) => next_sequence_record(r),
            Self::LossyBinnedSplitSequence(r) => next_sequence_record(r),
            Self::RawRawRawQuality(r) => next_quality_record(r),
            Self::TwoBitRawRawQuality(r) => next_quality_record(r),
            Self::TwoBitBinnedSplitQuality(r) => next_quality_record(r),
            Self::LossyBinnedSplitQuality(r) => next_quality_record(r),
            Self::RawRawRawB8Key(r) => next_key_record(r),
            Self::TwoBitBinnedSplitB8Key(r) => next_key_record(r),
            Self::RawRawRawB8SequenceKey(r) => next_sequence_key_record(r),
            Self::TwoBitBinnedSplitB8SequenceKey(r) => next_sequence_key_record(r),
        }
    }
}

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
                    .sequence_codec::<TwoBitLossyNCodec>()
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
                    self.sequence_codec, self.quality_codec, self.name_codec, self.record_key,
                )));
            },
        };

        Ok(Writer { inner: Some(inner) })
    }
}

#[pyclass]
struct Writer {
    inner: Option<WriterInner>,
}

#[pymethods]
impl Writer {
    #[staticmethod]
    fn builder() -> WriterBuilder {
        WriterBuilder::new()
    }

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

#[pyclass]
#[derive(Clone)]
struct Record {
    #[pyo3(get)]
    name: Option<Vec<u8>>,
    #[pyo3(get)]
    sequence: Option<Vec<u8>>,
    #[pyo3(get)]
    quality: Option<Vec<u8>>,
    #[pyo3(get)]
    key: Option<Vec<u8>>,
}

impl Record {
    fn full(name: Vec<u8>, sequence: Vec<u8>, quality: Vec<u8>, key: Option<Vec<u8>>) -> Self {
        Self {
            name: Some(name),
            sequence: Some(sequence),
            quality: Some(quality),
            key,
        }
    }

    fn name_only(name: Vec<u8>) -> Self {
        Self {
            name: Some(name),
            sequence: None,
            quality: None,
            key: None,
        }
    }

    fn sequence_only(sequence: Vec<u8>) -> Self {
        Self {
            name: None,
            sequence: Some(sequence),
            quality: None,
            key: None,
        }
    }

    fn quality_only(quality: Vec<u8>) -> Self {
        Self {
            name: None,
            sequence: None,
            quality: Some(quality),
            key: None,
        }
    }

    fn key_only(key: Vec<u8>) -> Self {
        Self {
            name: None,
            sequence: None,
            quality: None,
            key: Some(key),
        }
    }

    fn sequence_and_key(sequence: Vec<u8>, key: Vec<u8>) -> Self {
        Self {
            name: None,
            sequence: Some(sequence),
            quality: None,
            key: Some(key),
        }
    }
}

#[pymethods]
impl Record {
    fn __repr__(&self) -> String {
        let name = self
            .name
            .as_deref()
            .map(String::from_utf8_lossy)
            .unwrap_or_else(|| std::borrow::Cow::Borrowed("<unselected>"));
        let sequence_len = self.sequence.as_ref().map_or(0, Vec::len);
        format!("Record(name={name}, len={sequence_len})")
    }
}

#[pyclass]
struct ReaderBuilder {
    sequence_codec: String,
    quality_codec: String,
    name_codec: String,
    record_key: String,
    selected_fields: Vec<String>,
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
            selected_fields: Vec::new(),
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

    fn project_name(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        push_selected_field(&mut slf.selected_fields, "name");
        slf
    }

    fn project_sequence(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        push_selected_field(&mut slf.selected_fields, "sequence");
        slf
    }

    fn project_quality(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        push_selected_field(&mut slf.selected_fields, "quality");
        slf
    }

    fn project_key(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        push_selected_field(&mut slf.selected_fields, "key");
        slf
    }

    fn build(&self, data: Vec<u8>) -> PyResult<Reader> {
        let cursor = std::io::Cursor::new(data);
        let projection = parse_projection(&self.selected_fields, self.record_key.as_str())?;
        let inner = build_reader_inner(
            cursor,
            (
                self.sequence_codec.as_str(),
                self.quality_codec.as_str(),
                self.name_codec.as_str(),
                self.record_key.as_str(),
            ),
            projection,
        )?;
        Ok(Reader { inner })
    }
}

#[pyclass]
struct Reader {
    inner: ReaderKind,
}

#[pymethods]
impl Reader {
    #[staticmethod]
    fn builder() -> ReaderBuilder {
        ReaderBuilder::new()
    }

    #[staticmethod]
    fn open(data: Vec<u8>) -> PyResult<Self> {
        let cursor = std::io::Cursor::new(data);
        let inner = ReaderKind::Full(ReaderInner::RawRawRaw(
            RustReader::new(cursor).map_err(to_py_err)?,
        ));
        Ok(Self { inner })
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self) -> PyResult<Option<Record>> {
        match &mut self.inner {
            ReaderKind::Full(inner) => {
                let has_record = inner.next_record().map_err(to_py_err)?;
                if !has_record {
                    return Ok(None);
                }

                let name = inner.name().to_vec();
                let sequence = inner.sequence().to_vec();
                let quality = inner.quality().to_vec();
                let key = inner.record_key().map_err(to_py_err)?;

                Ok(Some(Record::full(name, sequence, quality, key)))
            },
            ReaderKind::Selected(inner) => inner
                .next_projected_record()
                .map(|projected| {
                    projected.map(|record| match record {
                        ProjectedRecordData::Name(name) => Record::name_only(name),
                        ProjectedRecordData::Sequence(sequence) => Record::sequence_only(sequence),
                        ProjectedRecordData::Quality(quality) => Record::quality_only(quality),
                        ProjectedRecordData::Key(key) => Record::key_only(key),
                        ProjectedRecordData::SequenceKey { sequence, key } => {
                            Record::sequence_and_key(sequence, key)
                        },
                    })
                })
                .map_err(to_py_err),
        }
    }
}

enum ReaderKind {
    Full(ReaderInner),
    Selected(SelectedReaderInner),
}

#[pyfunction]
#[pyo3(signature = (
    data,
    projection,
    sequence_codec = "raw",
    quality_codec = "raw",
    name_codec = "raw",
    record_key = "none"
))]
fn open_projected(
    data: Vec<u8>,
    projection: &str,
    sequence_codec: &str,
    quality_codec: &str,
    name_codec: &str,
    record_key: &str,
) -> PyResult<Reader> {
    let cursor = std::io::Cursor::new(data);
    let projection = parse_projection_name(projection, record_key)?;
    let inner = build_reader_inner(
        cursor,
        (sequence_codec, quality_codec, name_codec, record_key),
        projection,
    )?;
    Ok(Reader { inner })
}

fn parse_projection(fields: &[String], key_kind: &str) -> PyResult<Projection> {
    validate_selected_field_names(fields)?;

    if fields.is_empty() {
        return Ok(Projection::All);
    }

    let mut name = false;
    let mut sequence = false;
    let mut quality = false;
    let mut key = false;

    for field in fields {
        match field.as_str() {
            "name" => name = true,
            "sequence" => sequence = true,
            "quality" => quality = true,
            "key" => key = true,
            _ => unreachable!("field names are pre-validated"),
        }
    }

    if key && key_kind == "none" {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            "key projection requires a keyed reader",
        ));
    }

    match (name, sequence, quality, key) {
        (true, true, true, false) => Ok(Projection::All),
        (true, false, false, false) => Ok(Projection::Name),
        (false, true, false, false) => Ok(Projection::Sequence),
        (false, false, true, false) => Ok(Projection::Quality),
        (false, false, false, true) => Ok(Projection::Key),
        (false, true, false, true) => Ok(Projection::SequenceKey),
        _ => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            "unsupported projection; supported projections are name, sequence, quality, key, sequence+key, or full row selection",
        )),
    }
}

fn parse_projection_name(name: &str, key_kind: &str) -> PyResult<Projection> {
    let fields = match name {
        "all" => vec![
            "name".to_string(),
            "sequence".to_string(),
            "quality".to_string(),
        ],
        "name" => vec!["name".to_string()],
        "sequence" => vec!["sequence".to_string()],
        "quality" => vec!["quality".to_string()],
        "key" => vec!["key".to_string()],
        "sequence+key" => vec!["sequence".to_string(), "key".to_string()],
        _ => {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "unsupported projection; expected one of: all, name, sequence, quality, key, sequence+key",
            ));
        },
    };

    parse_projection(&fields, key_kind)
}

fn build_reader_inner(
    cursor: R,
    codec_key: (&str, &str, &str, &str),
    projection: Projection,
) -> PyResult<ReaderKind> {
    match (codec_key, projection) {
        (("raw", "raw", "raw", "none"), Projection::All) => Ok(ReaderKind::Full(
            ReaderInner::RawRawRaw(RustReader::new(cursor).map_err(to_py_err)?),
        )),
        (("raw", "raw", "raw", "none"), Projection::Name) => {
            Ok(ReaderKind::Selected(SelectedReaderInner::RawRawRawName(
                RustReader::builder()
                    .inner(cursor)
                    .select(SelectName)
                    .build()
                    .map_err(to_py_err)?,
            )))
        },
        (("raw", "raw", "raw", "none"), Projection::Sequence) => Ok(ReaderKind::Selected(
            SelectedReaderInner::RawRawRawSequence(
                RustReader::builder()
                    .inner(cursor)
                    .select(SelectSequence)
                    .build()
                    .map_err(to_py_err)?,
            ),
        )),
        (("raw", "raw", "raw", "none"), Projection::Quality) => {
            Ok(ReaderKind::Selected(SelectedReaderInner::RawRawRawQuality(
                RustReader::builder()
                    .inner(cursor)
                    .select(SelectQuality)
                    .build()
                    .map_err(to_py_err)?,
            )))
        },
        (("two_bit_exact", "raw", "raw", "none"), Projection::All) => Ok(ReaderKind::Full(
            ReaderInner::TwoBitRawRaw(RustReader::with_two_bit_exact(cursor).map_err(to_py_err)?),
        )),
        (("two_bit_exact", "raw", "raw", "none"), Projection::Name) => {
            Ok(ReaderKind::Selected(SelectedReaderInner::TwoBitRawRawName(
                RustReader::builder()
                    .inner(cursor)
                    .two_bit_exact()
                    .select(SelectName)
                    .build()
                    .map_err(to_py_err)?,
            )))
        },
        (("two_bit_exact", "raw", "raw", "none"), Projection::Sequence) => Ok(
            ReaderKind::Selected(SelectedReaderInner::TwoBitRawRawSequence(
                RustReader::builder()
                    .inner(cursor)
                    .two_bit_exact()
                    .select(SelectSequence)
                    .build()
                    .map_err(to_py_err)?,
            )),
        ),
        (("two_bit_exact", "raw", "raw", "none"), Projection::Quality) => Ok(ReaderKind::Selected(
            SelectedReaderInner::TwoBitRawRawQuality(
                RustReader::builder()
                    .inner(cursor)
                    .two_bit_exact()
                    .select(SelectQuality)
                    .build()
                    .map_err(to_py_err)?,
            ),
        )),
        (("two_bit_exact", "binned", "split", "none"), Projection::All) => {
            Ok(ReaderKind::Full(ReaderInner::TwoBitBinnedSplit(
                RustReader::with_codecs::<TwoBitExactCodec, BinnedQualityCodec, SplitNameCodec>(
                    cursor,
                )
                .map_err(to_py_err)?,
            )))
        },
        (("two_bit_exact", "binned", "split", "none"), Projection::Name) => Ok(
            ReaderKind::Selected(SelectedReaderInner::TwoBitBinnedSplitName(
                RustReader::builder()
                    .inner(cursor)
                    .two_bit_exact()
                    .quality_codec::<BinnedQualityCodec>()
                    .name_codec::<SplitNameCodec>()
                    .select(SelectName)
                    .build()
                    .map_err(to_py_err)?,
            )),
        ),
        (("two_bit_exact", "binned", "split", "none"), Projection::Sequence) => Ok(
            ReaderKind::Selected(SelectedReaderInner::TwoBitBinnedSplitSequence(
                RustReader::builder()
                    .inner(cursor)
                    .two_bit_exact()
                    .quality_codec::<BinnedQualityCodec>()
                    .name_codec::<SplitNameCodec>()
                    .select(SelectSequence)
                    .build()
                    .map_err(to_py_err)?,
            )),
        ),
        (("two_bit_exact", "binned", "split", "none"), Projection::Quality) => Ok(
            ReaderKind::Selected(SelectedReaderInner::TwoBitBinnedSplitQuality(
                RustReader::builder()
                    .inner(cursor)
                    .two_bit_exact()
                    .quality_codec::<BinnedQualityCodec>()
                    .name_codec::<SplitNameCodec>()
                    .select(SelectQuality)
                    .build()
                    .map_err(to_py_err)?,
            )),
        ),
        (("two_bit_lossy_n", "binned", "split", "none"), Projection::All) => {
            Ok(ReaderKind::Full(ReaderInner::LossyBinnedSplit(
                RustReader::with_codecs::<TwoBitLossyNCodec, BinnedQualityCodec, SplitNameCodec>(
                    cursor,
                )
                .map_err(to_py_err)?,
            )))
        },
        (("two_bit_lossy_n", "binned", "split", "none"), Projection::Name) => Ok(
            ReaderKind::Selected(SelectedReaderInner::LossyBinnedSplitName(
                RustReader::builder()
                    .inner(cursor)
                    .sequence_codec::<TwoBitLossyNCodec>()
                    .quality_codec::<BinnedQualityCodec>()
                    .name_codec::<SplitNameCodec>()
                    .select(SelectName)
                    .build()
                    .map_err(to_py_err)?,
            )),
        ),
        (("two_bit_lossy_n", "binned", "split", "none"), Projection::Sequence) => Ok(
            ReaderKind::Selected(SelectedReaderInner::LossyBinnedSplitSequence(
                RustReader::builder()
                    .inner(cursor)
                    .sequence_codec::<TwoBitLossyNCodec>()
                    .quality_codec::<BinnedQualityCodec>()
                    .name_codec::<SplitNameCodec>()
                    .select(SelectSequence)
                    .build()
                    .map_err(to_py_err)?,
            )),
        ),
        (("two_bit_lossy_n", "binned", "split", "none"), Projection::Quality) => Ok(
            ReaderKind::Selected(SelectedReaderInner::LossyBinnedSplitQuality(
                RustReader::builder()
                    .inner(cursor)
                    .sequence_codec::<TwoBitLossyNCodec>()
                    .quality_codec::<BinnedQualityCodec>()
                    .name_codec::<SplitNameCodec>()
                    .select(SelectQuality)
                    .build()
                    .map_err(to_py_err)?,
            )),
        ),
        (("raw", "raw", "raw", "bytes8"), Projection::All) => Ok(ReaderKind::Full(
            ReaderInner::RawRawRawB8(RustReader::with_bytes8_key(cursor).map_err(to_py_err)?),
        )),
        (("raw", "raw", "raw", "bytes8"), Projection::Key) => {
            Ok(ReaderKind::Selected(SelectedReaderInner::RawRawRawB8Key(
                RustReader::builder()
                    .inner(cursor)
                    .bytes8_key()
                    .select(SelectKey)
                    .build()
                    .map_err(to_py_err)?,
            )))
        },
        (("raw", "raw", "raw", "bytes8"), Projection::SequenceKey) => Ok(ReaderKind::Selected(
            SelectedReaderInner::RawRawRawB8SequenceKey(
                RustReader::builder()
                    .inner(cursor)
                    .bytes8_key()
                    .select(SelectSequenceKey)
                    .build()
                    .map_err(to_py_err)?,
            ),
        )),
        (("two_bit_exact", "binned", "split", "bytes8"), Projection::All) => {
            Ok(ReaderKind::Full(ReaderInner::TwoBitBinnedSplitB8(
                RustReader::builder()
                    .inner(cursor)
                    .two_bit_exact()
                    .quality_codec::<BinnedQualityCodec>()
                    .name_codec::<SplitNameCodec>()
                    .bytes8_key()
                    .build()
                    .map_err(to_py_err)?,
            )))
        },
        (("two_bit_exact", "binned", "split", "bytes8"), Projection::Key) => Ok(
            ReaderKind::Selected(SelectedReaderInner::TwoBitBinnedSplitB8Key(
                RustReader::builder()
                    .inner(cursor)
                    .two_bit_exact()
                    .quality_codec::<BinnedQualityCodec>()
                    .name_codec::<SplitNameCodec>()
                    .bytes8_key()
                    .select(SelectKey)
                    .build()
                    .map_err(to_py_err)?,
            )),
        ),
        (("two_bit_exact", "binned", "split", "bytes8"), Projection::SequenceKey) => Ok(
            ReaderKind::Selected(SelectedReaderInner::TwoBitBinnedSplitB8SequenceKey(
                RustReader::builder()
                    .inner(cursor)
                    .two_bit_exact()
                    .quality_codec::<BinnedQualityCodec>()
                    .name_codec::<SplitNameCodec>()
                    .bytes8_key()
                    .select(SelectSequenceKey)
                    .build()
                    .map_err(to_py_err)?,
            )),
        ),
        ((seq, qual, name, key), _) => {
            Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "unsupported codec/projection combination: seq={seq}, qual={qual}, name={name}, key={key}",
            )))
        },
    }
}

fn validate_selected_field_names(fields: &[String]) -> PyResult<()> {
    for field in fields {
        match field.as_str() {
            "name" | "sequence" | "quality" | "key" => {},
            _ => {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "unknown selected field: {field}",
                )));
            },
        }
    }
    Ok(())
}

fn push_selected_field(fields: &mut Vec<String>, field: &str) {
    if !fields.iter().any(|existing| existing == field) {
        fields.push(field.to_string());
    }
}

#[pymodule]
fn dryice_python(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<WriterBuilder>()?;
    m.add_class::<Writer>()?;
    m.add_class::<ReaderBuilder>()?;
    m.add_class::<Reader>()?;
    m.add_class::<Record>()?;
    m.add_function(wrap_pyfunction!(open_projected, m)?)?;
    Ok(())
}

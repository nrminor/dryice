//! Node.js/TypeScript bindings for the `dryice` high-throughput genomic record container.

use napi::bindgen_prelude::*;
use napi_derive::napi;

use dryice::{
    BinnedQualityCodec, RawAsciiCodec, RawNameCodec, RawQualityCodec, SplitNameCodec,
    TwoBitExactCodec, TwoBitLossyNCodec,
};
use dryice::{
    Bytes8Key, DryIceError, DryIceReader as RustReader, DryIceWriter as RustWriter, NoRecordKey,
    SeqRecordLike,
};

fn to_napi_err(e: DryIceError) -> napi::Error {
    napi::Error::from_reason(e.to_string())
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
            WriterInner::RawRawRaw(w) => w.$method($($arg),*).map_err(to_napi_err),
            WriterInner::TwoBitRawRaw(w) => w.$method($($arg),*).map_err(to_napi_err),
            WriterInner::TwoBitBinnedSplit(w) => w.$method($($arg),*).map_err(to_napi_err),
            WriterInner::LossyBinnedSplit(w) => w.$method($($arg),*).map_err(to_napi_err),
            WriterInner::RawRawRawB8(w) => w.$method($($arg),*).map_err(to_napi_err),
            WriterInner::TwoBitBinnedSplitB8(w) => w.$method($($arg),*).map_err(to_napi_err),
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
    fn write_record(&mut self, record: &SliceRecord<'_>) -> Result<()> {
        match self {
            Self::RawRawRaw(w) => w.write_record(record).map_err(to_napi_err),
            Self::TwoBitRawRaw(w) => w.write_record(record).map_err(to_napi_err),
            Self::TwoBitBinnedSplit(w) => w.write_record(record).map_err(to_napi_err),
            Self::LossyBinnedSplit(w) => w.write_record(record).map_err(to_napi_err),
            Self::RawRawRawB8(_) | Self::TwoBitBinnedSplitB8(_) => Err(napi::Error::from_reason(
                "use writeRecordWithKey for keyed writers",
            )),
        }
    }

    fn write_record_with_key(&mut self, record: &SliceRecord<'_>, key: &[u8]) -> Result<()> {
        match self {
            Self::RawRawRawB8(w) => {
                let k = Bytes8Key(
                    key.try_into()
                        .map_err(|_| napi::Error::from_reason("key must be exactly 8 bytes"))?,
                );
                w.write_record_with_key(record, &k).map_err(to_napi_err)
            },
            Self::TwoBitBinnedSplitB8(w) => {
                let k = Bytes8Key(
                    key.try_into()
                        .map_err(|_| napi::Error::from_reason("key must be exactly 8 bytes"))?,
                );
                w.write_record_with_key(record, &k).map_err(to_napi_err)
            },
            _ => Err(napi::Error::from_reason(
                "writeRecordWithKey requires a keyed writer",
            )),
        }
    }

    fn finish(self) -> Result<Vec<u8>> {
        dispatch_all_writers!(self, finish())
    }
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
}

impl ReaderInner {
    fn next_record(&mut self) -> Result<bool> {
        dispatch_all_readers!(self, next_record()).map_err(to_napi_err)
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

    fn record_key(&self) -> Result<Option<Vec<u8>>> {
        match self {
            Self::RawRawRawB8(r) => {
                let k = r.record_key().map_err(to_napi_err)?;
                Ok(Some(k.0.to_vec()))
            },
            _ => Ok(None),
        }
    }
}

fn build_writer(
    seq: &str,
    qual: &str,
    name: &str,
    key: &str,
    block_records: u32,
) -> Result<WriterInner> {
    let n = block_records as usize;
    match (seq, qual, name, key) {
        ("raw", "raw", "raw", "none") => Ok(WriterInner::RawRawRaw(
            RustWriter::builder()
                .inner(Vec::new())
                .target_block_records(n)
                .build(),
        )),
        ("two_bit_exact", "raw", "raw", "none") => Ok(WriterInner::TwoBitRawRaw(
            RustWriter::builder()
                .inner(Vec::new())
                .two_bit_exact()
                .target_block_records(n)
                .build(),
        )),
        ("two_bit_exact", "binned", "split", "none") => Ok(WriterInner::TwoBitBinnedSplit(
            RustWriter::builder()
                .inner(Vec::new())
                .two_bit_exact()
                .binned_quality()
                .split_names()
                .target_block_records(n)
                .build(),
        )),
        ("two_bit_lossy_n", "binned", "split", "none") => Ok(WriterInner::LossyBinnedSplit(
            RustWriter::builder()
                .inner(Vec::new())
                .two_bit_lossy_n()
                .binned_quality()
                .split_names()
                .target_block_records(n)
                .build(),
        )),
        ("raw", "raw", "raw", "bytes8") => Ok(WriterInner::RawRawRawB8(
            RustWriter::builder()
                .inner(Vec::new())
                .bytes8_key()
                .target_block_records(n)
                .build(),
        )),
        ("two_bit_exact", "binned", "split", "bytes8") => Ok(WriterInner::TwoBitBinnedSplitB8(
            RustWriter::builder()
                .inner(Vec::new())
                .two_bit_exact()
                .binned_quality()
                .split_names()
                .bytes8_key()
                .target_block_records(n)
                .build(),
        )),
        _ => Err(napi::Error::from_reason(format!(
            "unsupported codec combination: seq={seq}, qual={qual}, name={name}, key={key}"
        ))),
    }
}

fn build_reader(
    data: Vec<u8>,
    seq: &str,
    qual: &str,
    name: &str,
    key: &str,
) -> Result<ReaderInner> {
    let cursor = std::io::Cursor::new(data);
    match (seq, qual, name, key) {
        ("raw", "raw", "raw", "none") => Ok(ReaderInner::RawRawRaw(
            RustReader::new(cursor).map_err(to_napi_err)?,
        )),
        ("two_bit_exact", "raw", "raw", "none") => Ok(ReaderInner::TwoBitRawRaw(
            RustReader::with_two_bit_exact(cursor).map_err(to_napi_err)?,
        )),
        ("two_bit_exact", "binned", "split", "none") => Ok(ReaderInner::TwoBitBinnedSplit(
            RustReader::with_codecs::<TwoBitExactCodec, BinnedQualityCodec, SplitNameCodec>(cursor)
                .map_err(to_napi_err)?,
        )),
        ("two_bit_lossy_n", "binned", "split", "none") => Ok(ReaderInner::LossyBinnedSplit(
            RustReader::with_codecs::<TwoBitLossyNCodec, BinnedQualityCodec, SplitNameCodec>(
                cursor,
            )
            .map_err(to_napi_err)?,
        )),
        ("raw", "raw", "raw", "bytes8") => Ok(ReaderInner::RawRawRawB8(
            RustReader::with_bytes8_key(cursor).map_err(to_napi_err)?,
        )),
        _ => Err(napi::Error::from_reason(format!(
            "unsupported codec combination: seq={seq}, qual={qual}, name={name}, key={key}"
        ))),
    }
}

#[napi(object)]
pub struct Record {
    pub name: Buffer,
    pub sequence: Buffer,
    pub quality: Buffer,
    pub key: Option<Buffer>,
}

#[napi]
pub struct WriterBuilder {
    sequence_codec: String,
    quality_codec: String,
    name_codec: String,
    record_key: String,
    target_block_records: u32,
}

impl Default for WriterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[napi]
impl WriterBuilder {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            sequence_codec: "raw".to_string(),
            quality_codec: "raw".to_string(),
            name_codec: "raw".to_string(),
            record_key: "none".to_string(),
            target_block_records: 8192,
        }
    }

    #[napi]
    pub fn two_bit_exact(&mut self) -> &Self {
        self.sequence_codec = "two_bit_exact".to_string();
        self
    }

    #[napi]
    pub fn two_bit_lossy_n(&mut self) -> &Self {
        self.sequence_codec = "two_bit_lossy_n".to_string();
        self
    }

    #[napi]
    pub fn binned_quality(&mut self) -> &Self {
        self.quality_codec = "binned".to_string();
        self
    }

    #[napi]
    pub fn split_names(&mut self) -> &Self {
        self.name_codec = "split".to_string();
        self
    }

    #[napi]
    pub fn bytes8_key(&mut self) -> &Self {
        self.record_key = "bytes8".to_string();
        self
    }

    #[napi]
    pub fn target_block_records(&mut self, n: u32) -> &Self {
        self.target_block_records = n;
        self
    }

    #[napi]
    pub fn build(&self) -> Result<Writer> {
        let inner = build_writer(
            &self.sequence_codec,
            &self.quality_codec,
            &self.name_codec,
            &self.record_key,
            self.target_block_records,
        )?;
        Ok(Writer { inner: Some(inner) })
    }
}

#[napi]
pub struct Writer {
    inner: Option<WriterInner>,
}

#[napi]
impl Writer {
    #[napi]
    pub fn write_record(&mut self, name: Buffer, sequence: Buffer, quality: Buffer) -> Result<()> {
        let record = SliceRecord {
            name: &name,
            sequence: &sequence,
            quality: &quality,
        };
        self.inner
            .as_mut()
            .ok_or_else(|| napi::Error::from_reason("writer already finished"))?
            .write_record(&record)
    }

    #[napi]
    pub fn write_record_with_key(
        &mut self,
        name: Buffer,
        sequence: Buffer,
        quality: Buffer,
        key: Buffer,
    ) -> Result<()> {
        let record = SliceRecord {
            name: &name,
            sequence: &sequence,
            quality: &quality,
        };
        self.inner
            .as_mut()
            .ok_or_else(|| napi::Error::from_reason("writer already finished"))?
            .write_record_with_key(&record, &key)
    }

    #[napi]
    pub fn finish(&mut self) -> Result<Buffer> {
        let data = self
            .inner
            .take()
            .ok_or_else(|| napi::Error::from_reason("writer already finished"))?
            .finish()?;
        Ok(data.into())
    }
}

#[napi]
pub struct ReaderBuilder {
    sequence_codec: String,
    quality_codec: String,
    name_codec: String,
    record_key: String,
}

impl Default for ReaderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[napi]
impl ReaderBuilder {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            sequence_codec: "raw".to_string(),
            quality_codec: "raw".to_string(),
            name_codec: "raw".to_string(),
            record_key: "none".to_string(),
        }
    }

    #[napi]
    pub fn two_bit_exact(&mut self) -> &Self {
        self.sequence_codec = "two_bit_exact".to_string();
        self
    }

    #[napi]
    pub fn two_bit_lossy_n(&mut self) -> &Self {
        self.sequence_codec = "two_bit_lossy_n".to_string();
        self
    }

    #[napi]
    pub fn binned_quality(&mut self) -> &Self {
        self.quality_codec = "binned".to_string();
        self
    }

    #[napi]
    pub fn split_names(&mut self) -> &Self {
        self.name_codec = "split".to_string();
        self
    }

    #[napi]
    pub fn bytes8_key(&mut self) -> &Self {
        self.record_key = "bytes8".to_string();
        self
    }

    #[napi]
    pub fn build(&self, data: Buffer) -> Result<Reader> {
        let inner = build_reader(
            data.to_vec(),
            &self.sequence_codec,
            &self.quality_codec,
            &self.name_codec,
            &self.record_key,
        )?;
        Ok(Reader { inner })
    }
}

#[napi]
pub struct Reader {
    inner: ReaderInner,
}

#[napi]
impl Reader {
    #[napi(factory)]
    pub fn open(data: Buffer) -> Result<Reader> {
        let inner = build_reader(data.to_vec(), "raw", "raw", "raw", "none")?;
        Ok(Reader { inner })
    }

    #[napi]
    pub fn next_record(&mut self) -> Result<Option<Record>> {
        let has_record = self.inner.next_record()?;
        if !has_record {
            return Ok(None);
        }

        let name = Buffer::from(self.inner.name());
        let sequence = Buffer::from(self.inner.sequence());
        let quality = Buffer::from(self.inner.quality());
        let key = self.inner.record_key()?.map(|k| Buffer::from(k.as_slice()));

        Ok(Some(Record {
            name,
            sequence,
            quality,
            key,
        }))
    }

    #[napi]
    pub fn records(&mut self) -> Result<Vec<Record>> {
        let mut records = Vec::new();
        while let Some(record) = self.next_record()? {
            records.push(record);
        }
        Ok(records)
    }
}

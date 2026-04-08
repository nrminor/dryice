//! Node.js/TypeScript bindings for the `dryice` high-throughput genomic record container.

use napi::bindgen_prelude::*;
use napi_derive::napi;

use dryice::{
    BinnedQualityCodec, DefaultMinimizer64, DefaultPrefixKmer64, OmittedNameCodec,
    OmittedQualityCodec, OmittedSequenceCodec, RawAsciiCodec, RawNameCodec, RawQualityCodec,
    SelectedDryIceReader as RustSelectedReader, SplitNameCodec, TwoBitExactCodec,
    TwoBitLossyNCodec,
    fields::{Key as SelectKey, Name as SelectName, Quality as SelectQuality},
    fields::{Sequence as SelectSequence, SequenceKey as SelectSequenceKey},
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
            WriterInner::RawOmitOmitB8(w) => w.$method($($arg),*).map_err(to_napi_err),
            WriterInner::RawSeqOnlyB8(w) => w.$method($($arg),*).map_err(to_napi_err),
            WriterInner::RawNameOnlyB8(w) => w.$method($($arg),*).map_err(to_napi_err),
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
            ReaderInner::RawOmitOmitB8(r) => r.$method($($arg),*),
            ReaderInner::RawSeqOnlyB8(r) => r.$method($($arg),*),
            ReaderInner::RawNameOnlyB8(r) => r.$method($($arg),*),
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
    RawOmitOmitB8(
        RustWriter<W, OmittedSequenceCodec, OmittedQualityCodec, OmittedNameCodec, Bytes8Key>,
    ),
    RawSeqOnlyB8(RustWriter<W, RawAsciiCodec, OmittedQualityCodec, OmittedNameCodec, Bytes8Key>),
    RawNameOnlyB8(
        RustWriter<W, OmittedSequenceCodec, OmittedQualityCodec, RawNameCodec, Bytes8Key>,
    ),
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
            Self::RawRawRawB8(_)
            | Self::RawOmitOmitB8(_)
            | Self::RawSeqOnlyB8(_)
            | Self::RawNameOnlyB8(_)
            | Self::TwoBitBinnedSplitB8(_) => Err(napi::Error::from_reason(
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
            Self::RawOmitOmitB8(w) => {
                let k = Bytes8Key(
                    key.try_into()
                        .map_err(|_| napi::Error::from_reason("key must be exactly 8 bytes"))?,
                );
                w.write_record_with_key(record, &k).map_err(to_napi_err)
            },
            Self::RawSeqOnlyB8(w) => {
                let k = Bytes8Key(
                    key.try_into()
                        .map_err(|_| napi::Error::from_reason("key must be exactly 8 bytes"))?,
                );
                w.write_record_with_key(record, &k).map_err(to_napi_err)
            },
            Self::RawNameOnlyB8(w) => {
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
    RawOmitOmitB8(
        RustReader<R, OmittedSequenceCodec, OmittedQualityCodec, OmittedNameCodec, Bytes8Key>,
    ),
    RawSeqOnlyB8(RustReader<R, RawAsciiCodec, OmittedQualityCodec, OmittedNameCodec, Bytes8Key>),
    RawNameOnlyB8(
        RustReader<R, OmittedSequenceCodec, OmittedQualityCodec, RawNameCodec, Bytes8Key>,
    ),
    TwoBitBinnedSplitB8(
        RustReader<R, TwoBitExactCodec, BinnedQualityCodec, SplitNameCodec, Bytes8Key>,
    ),
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
            Self::RawOmitOmitB8(r) => {
                let k = r.record_key().map_err(to_napi_err)?;
                Ok(Some(k.0.to_vec()))
            },
            Self::RawSeqOnlyB8(r) => {
                let k = r.record_key().map_err(to_napi_err)?;
                Ok(Some(k.0.to_vec()))
            },
            Self::RawNameOnlyB8(r) => {
                let k = r.record_key().map_err(to_napi_err)?;
                Ok(Some(k.0.to_vec()))
            },
            Self::TwoBitBinnedSplitB8(r) => {
                let k = r.record_key().map_err(to_napi_err)?;
                Ok(Some(k.0.to_vec()))
            },
            _ => Ok(None),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Projection {
    All,
    Name,
    Sequence,
    Quality,
    Key,
    SequenceKey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SequenceCodecKind {
    Raw,
    Omitted,
    TwoBitExact,
    TwoBitLossyN,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QualityCodecKind {
    Raw,
    Omitted,
    Binned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NameCodecKind {
    Raw,
    Omitted,
    Split,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KeyKind {
    None,
    Bytes8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CodecProfile {
    Raw,
    TwoBitExactRaw,
    TwoBitExactBinnedSplit,
    TwoBitLossyBinnedSplit,
    RawBytes8,
    RawOmitOmitBytes8,
    RawSeqOnlyBytes8,
    RawNameOnlyBytes8,
    TwoBitExactBinnedSplitBytes8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ReaderRequest {
    profile: CodecProfile,
    projection: Projection,
}

fn parse_sequence_codec(value: &str) -> Result<SequenceCodecKind> {
    match value {
        "raw" => Ok(SequenceCodecKind::Raw),
        "omitted" => Ok(SequenceCodecKind::Omitted),
        "two_bit_exact" => Ok(SequenceCodecKind::TwoBitExact),
        "two_bit_lossy_n" => Ok(SequenceCodecKind::TwoBitLossyN),
        _ => Err(napi::Error::from_reason(format!(
            "unknown sequence codec: {value}",
        ))),
    }
}

fn parse_quality_codec(value: &str) -> Result<QualityCodecKind> {
    match value {
        "raw" => Ok(QualityCodecKind::Raw),
        "omitted" => Ok(QualityCodecKind::Omitted),
        "binned" => Ok(QualityCodecKind::Binned),
        _ => Err(napi::Error::from_reason(format!(
            "unknown quality codec: {value}",
        ))),
    }
}

fn parse_name_codec(value: &str) -> Result<NameCodecKind> {
    match value {
        "raw" => Ok(NameCodecKind::Raw),
        "omitted" => Ok(NameCodecKind::Omitted),
        "split" => Ok(NameCodecKind::Split),
        _ => Err(napi::Error::from_reason(format!(
            "unknown name codec: {value}",
        ))),
    }
}

fn parse_key_kind(value: &str) -> Result<KeyKind> {
    match value {
        "none" => Ok(KeyKind::None),
        "bytes8" => Ok(KeyKind::Bytes8),
        _ => Err(napi::Error::from_reason(format!(
            "unknown key kind: {value}",
        ))),
    }
}

fn normalize_profile(
    sequence: SequenceCodecKind,
    quality: QualityCodecKind,
    name: NameCodecKind,
    key: KeyKind,
) -> Result<CodecProfile> {
    match (sequence, quality, name, key) {
        (SequenceCodecKind::Raw, QualityCodecKind::Raw, NameCodecKind::Raw, KeyKind::None) => {
            Ok(CodecProfile::Raw)
        },
        (
            SequenceCodecKind::TwoBitExact,
            QualityCodecKind::Raw,
            NameCodecKind::Raw,
            KeyKind::None,
        ) => Ok(CodecProfile::TwoBitExactRaw),
        (
            SequenceCodecKind::TwoBitExact,
            QualityCodecKind::Binned,
            NameCodecKind::Split,
            KeyKind::None,
        ) => Ok(CodecProfile::TwoBitExactBinnedSplit),
        (
            SequenceCodecKind::TwoBitLossyN,
            QualityCodecKind::Binned,
            NameCodecKind::Split,
            KeyKind::None,
        ) => Ok(CodecProfile::TwoBitLossyBinnedSplit),
        (SequenceCodecKind::Raw, QualityCodecKind::Raw, NameCodecKind::Raw, KeyKind::Bytes8) => {
            Ok(CodecProfile::RawBytes8)
        },
        (
            SequenceCodecKind::Omitted,
            QualityCodecKind::Omitted,
            NameCodecKind::Omitted,
            KeyKind::Bytes8,
        ) => Ok(CodecProfile::RawOmitOmitBytes8),
        (
            SequenceCodecKind::Raw,
            QualityCodecKind::Omitted,
            NameCodecKind::Omitted,
            KeyKind::Bytes8,
        ) => Ok(CodecProfile::RawSeqOnlyBytes8),
        (
            SequenceCodecKind::Omitted,
            QualityCodecKind::Omitted,
            NameCodecKind::Raw,
            KeyKind::Bytes8,
        ) => Ok(CodecProfile::RawNameOnlyBytes8),
        (
            SequenceCodecKind::TwoBitExact,
            QualityCodecKind::Binned,
            NameCodecKind::Split,
            KeyKind::Bytes8,
        ) => Ok(CodecProfile::TwoBitExactBinnedSplitBytes8),
        _ => Err(napi::Error::from_reason(
            "unsupported codec combination for the Node wrapper",
        )),
    }
}

impl ReaderRequest {
    fn from_builder(
        sequence_codec: &str,
        quality_codec: &str,
        name_codec: &str,
        record_key: &str,
        selected_fields: &[String],
    ) -> Result<Self> {
        let sequence = parse_sequence_codec(sequence_codec)?;
        let quality = parse_quality_codec(quality_codec)?;
        let name = parse_name_codec(name_codec)?;
        let key = parse_key_kind(record_key)?;
        let projection = parse_projection(selected_fields, key)?;
        let profile = normalize_profile(sequence, quality, name, key)?;
        Ok(Self {
            profile,
            projection,
        })
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

enum ReaderKind {
    Full(ReaderInner),
    Selected(SelectedReaderInner),
}

#[napi(object)]
pub struct Record {
    pub name: Option<Buffer>,
    pub sequence: Option<Buffer>,
    pub quality: Option<Buffer>,
    pub key: Option<Buffer>,
}

impl Record {
    fn full(name: &[u8], sequence: &[u8], quality: &[u8], key: Option<Vec<u8>>) -> Self {
        Self {
            name: Some(Buffer::from(name)),
            sequence: Some(Buffer::from(sequence)),
            quality: Some(Buffer::from(quality)),
            key: key.map(Buffer::from),
        }
    }

    fn name_only(name: &[u8]) -> Self {
        Self {
            name: Some(Buffer::from(name)),
            sequence: None,
            quality: None,
            key: None,
        }
    }

    fn sequence_only(sequence: &[u8]) -> Self {
        Self {
            name: None,
            sequence: Some(Buffer::from(sequence)),
            quality: None,
            key: None,
        }
    }

    fn quality_only(quality: &[u8]) -> Self {
        Self {
            name: None,
            sequence: None,
            quality: Some(Buffer::from(quality)),
            key: None,
        }
    }

    fn key_only(key: Vec<u8>) -> Self {
        Self {
            name: None,
            sequence: None,
            quality: None,
            key: Some(Buffer::from(key)),
        }
    }

    fn sequence_and_key(sequence: &[u8], key: Vec<u8>) -> Self {
        Self {
            name: None,
            sequence: Some(Buffer::from(sequence)),
            quality: None,
            key: Some(Buffer::from(key)),
        }
    }
}

macro_rules! dispatch_selected_readers {
    ($self:expr, { $( $variant:ident => $handler:ident ),* $(,)? }) => {
        match $self {
            $( Self::$variant(reader) => $handler(reader), )*
        }
    };
}

fn next_name_record<S, Q, N, K>(
    reader: &mut RustSelectedReader<R, S, Q, N, K, SelectName>,
) -> Result<Option<Record>>
where
    S: dryice::SequenceCodec,
    Q: dryice::QualityCodec,
    N: dryice::NameCodec,
{
    Ok(reader
        .next_record()
        .map_err(to_napi_err)?
        .map(|record| Record::name_only(record.name())))
}

fn next_sequence_record<S, Q, N, K>(
    reader: &mut RustSelectedReader<R, S, Q, N, K, SelectSequence>,
) -> Result<Option<Record>>
where
    S: dryice::SequenceCodec,
    Q: dryice::QualityCodec,
    N: dryice::NameCodec,
{
    Ok(reader
        .next_record()
        .map_err(to_napi_err)?
        .map(|record| Record::sequence_only(record.sequence())))
}

fn next_quality_record<S, Q, N, K>(
    reader: &mut RustSelectedReader<R, S, Q, N, K, SelectQuality>,
) -> Result<Option<Record>>
where
    S: dryice::SequenceCodec,
    Q: dryice::QualityCodec,
    N: dryice::NameCodec,
{
    Ok(reader
        .next_record()
        .map_err(to_napi_err)?
        .map(|record| Record::quality_only(record.quality())))
}

fn next_key_record<S, Q, N>(
    reader: &mut RustSelectedReader<R, S, Q, N, Bytes8Key, SelectKey>,
) -> Result<Option<Record>>
where
    S: dryice::SequenceCodec,
    Q: dryice::QualityCodec,
    N: dryice::NameCodec,
{
    if let Some(record) = reader.next_record().map_err(to_napi_err)? {
        Ok(Some(Record::key_only(
            record.record_key().map_err(to_napi_err)?.0.to_vec(),
        )))
    } else {
        Ok(None)
    }
}

fn next_sequence_key_record<S, Q, N>(
    reader: &mut RustSelectedReader<R, S, Q, N, Bytes8Key, SelectSequenceKey>,
) -> Result<Option<Record>>
where
    S: dryice::SequenceCodec,
    Q: dryice::QualityCodec,
    N: dryice::NameCodec,
{
    if let Some(record) = reader.next_record().map_err(to_napi_err)? {
        Ok(Some(Record::sequence_and_key(
            record.sequence(),
            record.record_key().map_err(to_napi_err)?.0.to_vec(),
        )))
    } else {
        Ok(None)
    }
}

impl SelectedReaderInner {
    fn next_record(&mut self) -> Result<Option<Record>> {
        dispatch_selected_readers!(self, {
            RawRawRawName => next_name_record,
            TwoBitRawRawName => next_name_record,
            TwoBitBinnedSplitName => next_name_record,
            LossyBinnedSplitName => next_name_record,
            RawRawRawSequence => next_sequence_record,
            TwoBitRawRawSequence => next_sequence_record,
            TwoBitBinnedSplitSequence => next_sequence_record,
            LossyBinnedSplitSequence => next_sequence_record,
            RawRawRawQuality => next_quality_record,
            TwoBitRawRawQuality => next_quality_record,
            TwoBitBinnedSplitQuality => next_quality_record,
            LossyBinnedSplitQuality => next_quality_record,
            RawRawRawB8Key => next_key_record,
            TwoBitBinnedSplitB8Key => next_key_record,
            RawRawRawB8SequenceKey => next_sequence_key_record,
            TwoBitBinnedSplitB8SequenceKey => next_sequence_key_record,
        })
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
        ("omitted", "omitted", "omitted", "bytes8") => Ok(WriterInner::RawOmitOmitB8(
            RustWriter::builder()
                .inner(Vec::new())
                .omit_sequence()
                .omit_quality()
                .omit_names()
                .bytes8_key()
                .target_block_records(n)
                .build(),
        )),
        ("raw", "omitted", "omitted", "bytes8") => Ok(WriterInner::RawSeqOnlyB8(
            RustWriter::builder()
                .inner(Vec::new())
                .omit_quality()
                .omit_names()
                .bytes8_key()
                .target_block_records(n)
                .build(),
        )),
        ("omitted", "omitted", "raw", "bytes8") => Ok(WriterInner::RawNameOnlyB8(
            RustWriter::builder()
                .inner(Vec::new())
                .omit_sequence()
                .omit_quality()
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

fn build_full_reader(data: Vec<u8>, profile: CodecProfile) -> Result<ReaderInner> {
    let cursor = std::io::Cursor::new(data);
    match profile {
        CodecProfile::Raw => Ok(ReaderInner::RawRawRaw(
            RustReader::new(cursor).map_err(to_napi_err)?,
        )),
        CodecProfile::TwoBitExactRaw => Ok(ReaderInner::TwoBitRawRaw(
            RustReader::with_two_bit_exact(cursor).map_err(to_napi_err)?,
        )),
        CodecProfile::TwoBitExactBinnedSplit => Ok(ReaderInner::TwoBitBinnedSplit(
            RustReader::with_codecs::<TwoBitExactCodec, BinnedQualityCodec, SplitNameCodec>(cursor)
                .map_err(to_napi_err)?,
        )),
        CodecProfile::TwoBitLossyBinnedSplit => Ok(ReaderInner::LossyBinnedSplit(
            RustReader::with_codecs::<TwoBitLossyNCodec, BinnedQualityCodec, SplitNameCodec>(
                cursor,
            )
            .map_err(to_napi_err)?,
        )),
        CodecProfile::RawBytes8 => Ok(ReaderInner::RawRawRawB8(
            RustReader::with_bytes8_key(cursor).map_err(to_napi_err)?,
        )),
        CodecProfile::RawOmitOmitBytes8 => Ok(ReaderInner::RawOmitOmitB8(
            RustReader::builder()
                .inner(cursor)
                .omit_sequence()
                .omit_quality()
                .omit_names()
                .bytes8_key()
                .build()
                .map_err(to_napi_err)?,
        )),
        CodecProfile::RawSeqOnlyBytes8 => Ok(ReaderInner::RawSeqOnlyB8(
            RustReader::builder()
                .inner(cursor)
                .omit_quality()
                .omit_names()
                .bytes8_key()
                .build()
                .map_err(to_napi_err)?,
        )),
        CodecProfile::RawNameOnlyBytes8 => Ok(ReaderInner::RawNameOnlyB8(
            RustReader::builder()
                .inner(cursor)
                .omit_sequence()
                .omit_quality()
                .bytes8_key()
                .build()
                .map_err(to_napi_err)?,
        )),
        CodecProfile::TwoBitExactBinnedSplitBytes8 => Ok(ReaderInner::TwoBitBinnedSplitB8(
            RustReader::builder()
                .inner(cursor)
                .two_bit_exact()
                .quality_codec::<BinnedQualityCodec>()
                .name_codec::<SplitNameCodec>()
                .bytes8_key()
                .build()
                .map_err(to_napi_err)?,
        )),
    }
}

fn validate_selected_fields(fields: &[String]) -> Result<()> {
    for field in fields {
        match field.as_str() {
            "name" | "sequence" | "quality" | "key" => {},
            _ => {
                return Err(napi::Error::from_reason(format!(
                    "unknown selected field: {field}",
                )));
            },
        }
    }
    Ok(())
}

fn parse_projection(fields: &[String], key_kind: KeyKind) -> Result<Projection> {
    validate_selected_fields(fields)?;

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

    if key && key_kind == KeyKind::None {
        return Err(napi::Error::from_reason(
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
        _ => Err(napi::Error::from_reason(
            "unsupported projection; supported projections are name, sequence, quality, key, sequence+key, or full row selection",
        )),
    }
}

fn build_selected_reader(
    data: Vec<u8>,
    profile: CodecProfile,
    projection: Projection,
) -> Result<SelectedReaderInner> {
    let cursor = std::io::Cursor::new(data);
    match (profile, projection) {
        (CodecProfile::Raw, Projection::Name) => Ok(SelectedReaderInner::RawRawRawName(
            RustReader::builder()
                .inner(cursor)
                .select(SelectName)
                .build()
                .map_err(to_napi_err)?,
        )),
        (CodecProfile::Raw, Projection::Sequence) => Ok(SelectedReaderInner::RawRawRawSequence(
            RustReader::builder()
                .inner(cursor)
                .select(SelectSequence)
                .build()
                .map_err(to_napi_err)?,
        )),
        (CodecProfile::Raw, Projection::Quality) => Ok(SelectedReaderInner::RawRawRawQuality(
            RustReader::builder()
                .inner(cursor)
                .select(SelectQuality)
                .build()
                .map_err(to_napi_err)?,
        )),
        (CodecProfile::TwoBitExactRaw, Projection::Name) => {
            Ok(SelectedReaderInner::TwoBitRawRawName(
                RustReader::builder()
                    .inner(cursor)
                    .two_bit_exact()
                    .select(SelectName)
                    .build()
                    .map_err(to_napi_err)?,
            ))
        },
        (CodecProfile::TwoBitExactRaw, Projection::Sequence) => {
            Ok(SelectedReaderInner::TwoBitRawRawSequence(
                RustReader::builder()
                    .inner(cursor)
                    .two_bit_exact()
                    .select(SelectSequence)
                    .build()
                    .map_err(to_napi_err)?,
            ))
        },
        (CodecProfile::TwoBitExactRaw, Projection::Quality) => {
            Ok(SelectedReaderInner::TwoBitRawRawQuality(
                RustReader::builder()
                    .inner(cursor)
                    .two_bit_exact()
                    .select(SelectQuality)
                    .build()
                    .map_err(to_napi_err)?,
            ))
        },
        (CodecProfile::TwoBitExactBinnedSplit, Projection::Name) => {
            Ok(SelectedReaderInner::TwoBitBinnedSplitName(
                RustReader::builder()
                    .inner(cursor)
                    .two_bit_exact()
                    .quality_codec::<BinnedQualityCodec>()
                    .name_codec::<SplitNameCodec>()
                    .select(SelectName)
                    .build()
                    .map_err(to_napi_err)?,
            ))
        },
        (CodecProfile::TwoBitExactBinnedSplit, Projection::Sequence) => {
            Ok(SelectedReaderInner::TwoBitBinnedSplitSequence(
                RustReader::builder()
                    .inner(cursor)
                    .two_bit_exact()
                    .quality_codec::<BinnedQualityCodec>()
                    .name_codec::<SplitNameCodec>()
                    .select(SelectSequence)
                    .build()
                    .map_err(to_napi_err)?,
            ))
        },
        (CodecProfile::TwoBitExactBinnedSplit, Projection::Quality) => {
            Ok(SelectedReaderInner::TwoBitBinnedSplitQuality(
                RustReader::builder()
                    .inner(cursor)
                    .two_bit_exact()
                    .quality_codec::<BinnedQualityCodec>()
                    .name_codec::<SplitNameCodec>()
                    .select(SelectQuality)
                    .build()
                    .map_err(to_napi_err)?,
            ))
        },
        (CodecProfile::TwoBitLossyBinnedSplit, Projection::Name) => {
            Ok(SelectedReaderInner::LossyBinnedSplitName(
                RustReader::builder()
                    .inner(cursor)
                    .sequence_codec::<TwoBitLossyNCodec>()
                    .quality_codec::<BinnedQualityCodec>()
                    .name_codec::<SplitNameCodec>()
                    .select(SelectName)
                    .build()
                    .map_err(to_napi_err)?,
            ))
        },
        (CodecProfile::TwoBitLossyBinnedSplit, Projection::Sequence) => {
            Ok(SelectedReaderInner::LossyBinnedSplitSequence(
                RustReader::builder()
                    .inner(cursor)
                    .sequence_codec::<TwoBitLossyNCodec>()
                    .quality_codec::<BinnedQualityCodec>()
                    .name_codec::<SplitNameCodec>()
                    .select(SelectSequence)
                    .build()
                    .map_err(to_napi_err)?,
            ))
        },
        (CodecProfile::TwoBitLossyBinnedSplit, Projection::Quality) => {
            Ok(SelectedReaderInner::LossyBinnedSplitQuality(
                RustReader::builder()
                    .inner(cursor)
                    .sequence_codec::<TwoBitLossyNCodec>()
                    .quality_codec::<BinnedQualityCodec>()
                    .name_codec::<SplitNameCodec>()
                    .select(SelectQuality)
                    .build()
                    .map_err(to_napi_err)?,
            ))
        },
        (CodecProfile::RawBytes8, Projection::Key) => Ok(SelectedReaderInner::RawRawRawB8Key(
            RustReader::builder()
                .inner(cursor)
                .bytes8_key()
                .select(SelectKey)
                .build()
                .map_err(to_napi_err)?,
        )),
        (CodecProfile::RawBytes8, Projection::SequenceKey) => {
            Ok(SelectedReaderInner::RawRawRawB8SequenceKey(
                RustReader::builder()
                    .inner(cursor)
                    .bytes8_key()
                    .select(SelectSequenceKey)
                    .build()
                    .map_err(to_napi_err)?,
            ))
        },
        (CodecProfile::TwoBitExactBinnedSplitBytes8, Projection::Key) => {
            Ok(SelectedReaderInner::TwoBitBinnedSplitB8Key(
                RustReader::builder()
                    .inner(cursor)
                    .two_bit_exact()
                    .quality_codec::<BinnedQualityCodec>()
                    .name_codec::<SplitNameCodec>()
                    .bytes8_key()
                    .select(SelectKey)
                    .build()
                    .map_err(to_napi_err)?,
            ))
        },
        (CodecProfile::TwoBitExactBinnedSplitBytes8, Projection::SequenceKey) => {
            Ok(SelectedReaderInner::TwoBitBinnedSplitB8SequenceKey(
                RustReader::builder()
                    .inner(cursor)
                    .two_bit_exact()
                    .quality_codec::<BinnedQualityCodec>()
                    .name_codec::<SplitNameCodec>()
                    .bytes8_key()
                    .select(SelectSequenceKey)
                    .build()
                    .map_err(to_napi_err)?,
            ))
        },
        (profile, projection) => Err(napi::Error::from_reason(format!(
            "unsupported projection {:?} for codec profile {:?}",
            projection, profile,
        ))),
    }
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
    pub fn prefix_kmers(&mut self) -> &Self {
        self.record_key = "bytes8".to_string();
        self.sequence_codec = "omitted".to_string();
        self.quality_codec = "omitted".to_string();
        self.name_codec = "omitted".to_string();
        self
    }

    #[napi]
    pub fn prefix_kmers_with_sequences(&mut self) -> &Self {
        self.record_key = "bytes8".to_string();
        self.sequence_codec = "raw".to_string();
        self.quality_codec = "omitted".to_string();
        self.name_codec = "omitted".to_string();
        self
    }

    #[napi]
    pub fn prefix_kmers_with_names(&mut self) -> &Self {
        self.record_key = "bytes8".to_string();
        self.sequence_codec = "omitted".to_string();
        self.quality_codec = "omitted".to_string();
        self.name_codec = "raw".to_string();
        self
    }

    #[napi]
    pub fn minimizers(&mut self) -> &Self {
        self.record_key = "bytes8".to_string();
        self.sequence_codec = "omitted".to_string();
        self.quality_codec = "omitted".to_string();
        self.name_codec = "omitted".to_string();
        self
    }

    #[napi]
    pub fn minimizers_with_sequences(&mut self) -> &Self {
        self.record_key = "bytes8".to_string();
        self.sequence_codec = "raw".to_string();
        self.quality_codec = "omitted".to_string();
        self.name_codec = "omitted".to_string();
        self
    }

    #[napi]
    pub fn minimizers_with_names(&mut self) -> &Self {
        self.record_key = "bytes8".to_string();
        self.sequence_codec = "omitted".to_string();
        self.quality_codec = "omitted".to_string();
        self.name_codec = "raw".to_string();
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
    selected_fields: Vec<String>,
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
            selected_fields: Vec::new(),
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
    pub fn prefix_kmers(&mut self) -> &Self {
        self.record_key = "bytes8".to_string();
        self.sequence_codec = "omitted".to_string();
        self.quality_codec = "omitted".to_string();
        self.name_codec = "omitted".to_string();
        self
    }

    #[napi]
    pub fn prefix_kmers_with_sequences(&mut self) -> &Self {
        self.record_key = "bytes8".to_string();
        self.sequence_codec = "raw".to_string();
        self.quality_codec = "omitted".to_string();
        self.name_codec = "omitted".to_string();
        self
    }

    #[napi]
    pub fn prefix_kmers_with_names(&mut self) -> &Self {
        self.record_key = "bytes8".to_string();
        self.sequence_codec = "omitted".to_string();
        self.quality_codec = "omitted".to_string();
        self.name_codec = "raw".to_string();
        self
    }

    #[napi]
    pub fn minimizers(&mut self) -> &Self {
        self.record_key = "bytes8".to_string();
        self.sequence_codec = "omitted".to_string();
        self.quality_codec = "omitted".to_string();
        self.name_codec = "omitted".to_string();
        self
    }

    #[napi]
    pub fn minimizers_with_sequences(&mut self) -> &Self {
        self.record_key = "bytes8".to_string();
        self.sequence_codec = "raw".to_string();
        self.quality_codec = "omitted".to_string();
        self.name_codec = "omitted".to_string();
        self
    }

    #[napi]
    pub fn minimizers_with_names(&mut self) -> &Self {
        self.record_key = "bytes8".to_string();
        self.sequence_codec = "omitted".to_string();
        self.quality_codec = "omitted".to_string();
        self.name_codec = "raw".to_string();
        self
    }

    #[napi]
    pub fn select(&mut self, fields: Vec<String>) -> Result<&Self> {
        validate_selected_fields(&fields)?;
        self.selected_fields = fields;
        Ok(self)
    }

    #[napi]
    pub fn build(&self, data: Buffer) -> Result<Reader> {
        let request = ReaderRequest::from_builder(
            &self.sequence_codec,
            &self.quality_codec,
            &self.name_codec,
            &self.record_key,
            &self.selected_fields,
        )?;

        let inner = match request.projection {
            Projection::All => ReaderKind::Full(build_full_reader(data.to_vec(), request.profile)?),
            projection => ReaderKind::Selected(build_selected_reader(
                data.to_vec(),
                request.profile,
                projection,
            )?),
        };
        Ok(Reader { inner })
    }
}

#[napi]
pub struct Reader {
    inner: ReaderKind,
}

#[napi]
impl Reader {
    #[napi(factory)]
    pub fn open(data: Buffer) -> Result<Reader> {
        let inner = ReaderKind::Full(build_full_reader(data.to_vec(), CodecProfile::Raw)?);
        Ok(Reader { inner })
    }

    #[napi]
    pub fn next_record(&mut self) -> Result<Option<Record>> {
        match &mut self.inner {
            ReaderKind::Full(inner) => {
                let has_record = inner.next_record()?;
                if !has_record {
                    return Ok(None);
                }

                let name = inner.name();
                let sequence = inner.sequence();
                let quality = inner.quality();
                let key = inner.record_key()?;

                Ok(Some(Record::full(name, sequence, quality, key)))
            },
            ReaderKind::Selected(inner) => inner.next_record(),
        }
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

#[napi]
pub fn default_prefix_kmer_key(sequence: Buffer) -> Result<Option<Buffer>> {
    Ok(DefaultPrefixKmer64::try_from_sequence(&sequence)
        .map_err(to_napi_err)?
        .map(|key| Buffer::from(key.0.to_le_bytes().to_vec())))
}

#[napi]
pub fn default_minimizer_key(sequence: Buffer) -> Result<Option<Buffer>> {
    Ok(DefaultMinimizer64::try_from_sequence(&sequence)
        .map_err(to_napi_err)?
        .map(|key| Buffer::from(key.0.to_le_bytes().to_vec())))
}

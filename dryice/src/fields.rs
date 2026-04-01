//! Field markers and selection-expression scaffolding for selected reads.
//!
//! The selection syntax is intentionally value-shaped for ergonomic builder
//! calls such as `select(Sequence | Key)`, but the result carries type-level
//! information so selected-read APIs can gate field access at compile time.

use std::ops::BitOr;

/// Public marker for read names.
#[derive(Debug, Clone, Copy, Default)]
pub struct Name;

/// Public marker for sequences.
#[derive(Debug, Clone, Copy, Default)]
pub struct Sequence;

/// Public marker for qualities.
#[derive(Debug, Clone, Copy, Default)]
pub struct Quality;

/// Public marker for record keys.
#[derive(Debug, Clone, Copy, Default)]
pub struct Key;

/// Selection set containing `Name | Sequence`.
#[derive(Debug, Clone, Copy, Default)]
pub struct NameSequence;

/// Selection set containing `Name | Quality`.
#[derive(Debug, Clone, Copy, Default)]
pub struct NameQuality;

/// Selection set containing `Name | Key`.
#[derive(Debug, Clone, Copy, Default)]
pub struct NameKey;

/// Selection set containing `Sequence | Quality`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SequenceQuality;

/// Selection set containing `Sequence | Key`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SequenceKey;

/// Selection set containing `Quality | Key`.
#[derive(Debug, Clone, Copy, Default)]
pub struct QualityKey;

/// Selection set containing `Name | Sequence | Quality`.
#[derive(Debug, Clone, Copy, Default)]
pub struct NameSequenceQuality;

/// Selection set containing `Name | Sequence | Key`.
#[derive(Debug, Clone, Copy, Default)]
pub struct NameSequenceKey;

/// Selection set containing `Name | Quality | Key`.
#[derive(Debug, Clone, Copy, Default)]
pub struct NameQualityKey;

/// Selection set containing `Sequence | Quality | Key`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SequenceQualityKey;

/// Selection set containing all supported fields.
#[derive(Debug, Clone, Copy, Default)]
pub struct AllFields;

/// Marker trait implemented by field-selection expressions.
pub trait SelectionExpr: private::Sealed + Copy {}

/// Internal preparation plan derived from a selected field set.
#[doc(hidden)]
pub trait SelectionPlan: SelectionExpr {
    const NEEDS_NAME: bool;
    const NEEDS_SEQUENCE: bool;
    const NEEDS_QUALITY: bool;
    const NEEDS_KEY: bool;
}

macro_rules! impl_selection_expr {
    ($($ty:ty),* $(,)?) => { $(impl SelectionExpr for $ty {})* };
}

impl_selection_expr!(
    Name,
    Sequence,
    Quality,
    Key,
    NameSequence,
    NameQuality,
    NameKey,
    SequenceQuality,
    SequenceKey,
    QualityKey,
    NameSequenceQuality,
    NameSequenceKey,
    NameQualityKey,
    SequenceQualityKey,
    AllFields,
);

#[doc(hidden)]
pub trait HasName: SelectionExpr {}

#[doc(hidden)]
pub trait HasSequence: SelectionExpr {}

#[doc(hidden)]
pub trait HasQuality: SelectionExpr {}

#[doc(hidden)]
pub trait HasKey: SelectionExpr {}

macro_rules! impl_selection_plan {
    ($ty:ty, $name:expr, $sequence:expr, $quality:expr, $key:expr) => {
        impl SelectionPlan for $ty {
            const NEEDS_NAME: bool = $name;
            const NEEDS_SEQUENCE: bool = $sequence;
            const NEEDS_QUALITY: bool = $quality;
            const NEEDS_KEY: bool = $key;
        }
    };
}

impl_selection_plan!(Name, true, false, false, false);
impl_selection_plan!(Sequence, false, true, false, false);
impl_selection_plan!(Quality, false, false, true, false);
impl_selection_plan!(Key, false, false, false, true);
impl_selection_plan!(NameSequence, true, true, false, false);
impl_selection_plan!(NameQuality, true, false, true, false);
impl_selection_plan!(NameKey, true, false, false, true);
impl_selection_plan!(SequenceQuality, false, true, true, false);
impl_selection_plan!(SequenceKey, false, true, false, true);
impl_selection_plan!(QualityKey, false, false, true, true);
impl_selection_plan!(NameSequenceQuality, true, true, true, false);
impl_selection_plan!(NameSequenceKey, true, true, false, true);
impl_selection_plan!(NameQualityKey, true, false, true, true);
impl_selection_plan!(SequenceQualityKey, false, true, true, true);
impl_selection_plan!(AllFields, true, true, true, true);

macro_rules! impl_has_name {
    ($($ty:ty),* $(,)?) => { $(impl HasName for $ty {})* };
}

macro_rules! impl_has_sequence {
    ($($ty:ty),* $(,)?) => { $(impl HasSequence for $ty {})* };
}

macro_rules! impl_has_quality {
    ($($ty:ty),* $(,)?) => { $(impl HasQuality for $ty {})* };
}

macro_rules! impl_has_key {
    ($($ty:ty),* $(,)?) => { $(impl HasKey for $ty {})* };
}

impl_has_name!(
    Name,
    NameSequence,
    NameQuality,
    NameKey,
    NameSequenceQuality,
    NameSequenceKey,
    NameQualityKey,
    AllFields,
);

impl_has_sequence!(
    Sequence,
    NameSequence,
    SequenceQuality,
    SequenceKey,
    NameSequenceQuality,
    NameSequenceKey,
    SequenceQualityKey,
    AllFields,
);

impl_has_quality!(
    Quality,
    NameQuality,
    SequenceQuality,
    QualityKey,
    NameSequenceQuality,
    NameQualityKey,
    SequenceQualityKey,
    AllFields,
);

impl_has_key!(
    Key,
    NameKey,
    SequenceKey,
    QualityKey,
    NameSequenceKey,
    NameQualityKey,
    SequenceQualityKey,
    AllFields,
);

macro_rules! impl_bitor {
    ($lhs:ty, $rhs:ty => $out:ty) => {
        impl BitOr<$rhs> for $lhs {
            type Output = $out;

            fn bitor(self, _rhs: $rhs) -> Self::Output {
                <$out>::default()
            }
        }
    };
}

impl_bitor!(Name, Name => Name);
impl_bitor!(Sequence, Sequence => Sequence);
impl_bitor!(Quality, Quality => Quality);
impl_bitor!(Key, Key => Key);

impl_bitor!(Name, Sequence => NameSequence);
impl_bitor!(Sequence, Name => NameSequence);
impl_bitor!(Name, Quality => NameQuality);
impl_bitor!(Quality, Name => NameQuality);
impl_bitor!(Name, Key => NameKey);
impl_bitor!(Key, Name => NameKey);
impl_bitor!(Sequence, Quality => SequenceQuality);
impl_bitor!(Quality, Sequence => SequenceQuality);
impl_bitor!(Sequence, Key => SequenceKey);
impl_bitor!(Key, Sequence => SequenceKey);
impl_bitor!(Quality, Key => QualityKey);
impl_bitor!(Key, Quality => QualityKey);

impl_bitor!(NameSequence, Name => NameSequence);
impl_bitor!(Name, NameSequence => NameSequence);
impl_bitor!(NameSequence, Quality => NameSequenceQuality);
impl_bitor!(Quality, NameSequence => NameSequenceQuality);
impl_bitor!(NameSequence, Key => NameSequenceKey);
impl_bitor!(Key, NameSequence => NameSequenceKey);

impl_bitor!(NameQuality, Name => NameQuality);
impl_bitor!(Name, NameQuality => NameQuality);
impl_bitor!(NameQuality, Sequence => NameSequenceQuality);
impl_bitor!(Sequence, NameQuality => NameSequenceQuality);
impl_bitor!(NameQuality, Key => NameQualityKey);
impl_bitor!(Key, NameQuality => NameQualityKey);

impl_bitor!(NameKey, Name => NameKey);
impl_bitor!(Name, NameKey => NameKey);
impl_bitor!(NameKey, Sequence => NameSequenceKey);
impl_bitor!(Sequence, NameKey => NameSequenceKey);
impl_bitor!(NameKey, Quality => NameQualityKey);
impl_bitor!(Quality, NameKey => NameQualityKey);

impl_bitor!(SequenceQuality, Sequence => SequenceQuality);
impl_bitor!(Sequence, SequenceQuality => SequenceQuality);
impl_bitor!(SequenceQuality, Name => NameSequenceQuality);
impl_bitor!(Name, SequenceQuality => NameSequenceQuality);
impl_bitor!(SequenceQuality, Key => SequenceQualityKey);
impl_bitor!(Key, SequenceQuality => SequenceQualityKey);

impl_bitor!(SequenceKey, Sequence => SequenceKey);
impl_bitor!(Sequence, SequenceKey => SequenceKey);
impl_bitor!(SequenceKey, Name => NameSequenceKey);
impl_bitor!(Name, SequenceKey => NameSequenceKey);
impl_bitor!(SequenceKey, Quality => SequenceQualityKey);
impl_bitor!(Quality, SequenceKey => SequenceQualityKey);

impl_bitor!(QualityKey, Quality => QualityKey);
impl_bitor!(Quality, QualityKey => QualityKey);
impl_bitor!(QualityKey, Name => NameQualityKey);
impl_bitor!(Name, QualityKey => NameQualityKey);
impl_bitor!(QualityKey, Sequence => SequenceQualityKey);
impl_bitor!(Sequence, QualityKey => SequenceQualityKey);

impl_bitor!(NameSequenceQuality, Name => NameSequenceQuality);
impl_bitor!(Name, NameSequenceQuality => NameSequenceQuality);
impl_bitor!(NameSequenceQuality, Sequence => NameSequenceQuality);
impl_bitor!(Sequence, NameSequenceQuality => NameSequenceQuality);
impl_bitor!(NameSequenceQuality, Quality => NameSequenceQuality);
impl_bitor!(Quality, NameSequenceQuality => NameSequenceQuality);
impl_bitor!(NameSequenceQuality, Key => AllFields);
impl_bitor!(Key, NameSequenceQuality => AllFields);

impl_bitor!(NameSequenceKey, Name => NameSequenceKey);
impl_bitor!(Name, NameSequenceKey => NameSequenceKey);
impl_bitor!(NameSequenceKey, Sequence => NameSequenceKey);
impl_bitor!(Sequence, NameSequenceKey => NameSequenceKey);
impl_bitor!(NameSequenceKey, Key => NameSequenceKey);
impl_bitor!(Key, NameSequenceKey => NameSequenceKey);
impl_bitor!(NameSequenceKey, Quality => AllFields);
impl_bitor!(Quality, NameSequenceKey => AllFields);

impl_bitor!(NameQualityKey, Name => NameQualityKey);
impl_bitor!(Name, NameQualityKey => NameQualityKey);
impl_bitor!(NameQualityKey, Quality => NameQualityKey);
impl_bitor!(Quality, NameQualityKey => NameQualityKey);
impl_bitor!(NameQualityKey, Key => NameQualityKey);
impl_bitor!(Key, NameQualityKey => NameQualityKey);
impl_bitor!(NameQualityKey, Sequence => AllFields);
impl_bitor!(Sequence, NameQualityKey => AllFields);

impl_bitor!(SequenceQualityKey, Sequence => SequenceQualityKey);
impl_bitor!(Sequence, SequenceQualityKey => SequenceQualityKey);
impl_bitor!(SequenceQualityKey, Quality => SequenceQualityKey);
impl_bitor!(Quality, SequenceQualityKey => SequenceQualityKey);
impl_bitor!(SequenceQualityKey, Key => SequenceQualityKey);
impl_bitor!(Key, SequenceQualityKey => SequenceQualityKey);
impl_bitor!(SequenceQualityKey, Name => AllFields);
impl_bitor!(Name, SequenceQualityKey => AllFields);

impl_bitor!(AllFields, Name => AllFields);
impl_bitor!(Name, AllFields => AllFields);
impl_bitor!(AllFields, Sequence => AllFields);
impl_bitor!(Sequence, AllFields => AllFields);
impl_bitor!(AllFields, Quality => AllFields);
impl_bitor!(Quality, AllFields => AllFields);
impl_bitor!(AllFields, Key => AllFields);
impl_bitor!(Key, AllFields => AllFields);

mod private {
    pub trait Sealed {}

    macro_rules! impl_sealed {
        ($($ty:ty),* $(,)?) => { $(impl Sealed for $ty {})* };
    }

    impl_sealed!(
        super::Name,
        super::Sequence,
        super::Quality,
        super::Key,
        super::NameSequence,
        super::NameQuality,
        super::NameKey,
        super::SequenceQuality,
        super::SequenceKey,
        super::QualityKey,
        super::NameSequenceQuality,
        super::NameSequenceKey,
        super::NameQualityKey,
        super::SequenceQualityKey,
        super::AllFields,
    );
}

#[cfg(test)]
mod tests {
    use super::{
        AllFields, HasKey, HasName, HasQuality, HasSequence, Key, Name, NameSequence,
        NameSequenceQuality, Quality, QualityKey, SelectionExpr, Sequence, SequenceKey,
    };

    fn assert_selection_expr<T: SelectionExpr>(_value: T) {}
    fn assert_has_name<T: HasName>(_value: T) {}
    fn assert_has_sequence<T: HasSequence>(_value: T) {}
    fn assert_has_quality<T: HasQuality>(_value: T) {}
    fn assert_has_key<T: HasKey>(_value: T) {}

    #[test]
    fn field_markers_compose_into_selection_sets() {
        let seq_key = Sequence | Key;
        let key_seq = Key | Sequence;
        let _: SequenceKey = seq_key;
        let _: SequenceKey = key_seq;

        let all = Sequence | Key | Quality | Name;
        let _: AllFields = all;
        assert_selection_expr(all);
    }

    #[test]
    fn capability_traits_match_selected_fields() {
        assert_has_sequence(Sequence);
        assert_has_name(NameSequence);
        assert_has_sequence(NameSequence);
        assert_has_quality(NameSequenceQuality);
        assert_has_key(QualityKey);
        assert_has_name(AllFields);
        assert_has_sequence(AllFields);
        assert_has_quality(AllFields);
        assert_has_key(AllFields);
    }
}

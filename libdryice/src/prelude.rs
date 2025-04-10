use std::{ops::Deref, path::Path};

use bio_seq::prelude::*;
use bitcode::{Decode, Encode};
use color_eyre::Result;
use log::warn;

pub trait SeqRecord {
    fn id(&self) -> &[u8];
    fn id_mut(&mut self) -> &mut Vec<u8>;
    fn sequence(&self) -> &[u8];
    fn sequence_mut(&mut self) -> &mut Vec<u8>;
    fn quality_scores(&self) -> Option<&[u8]> {
        None
    }
    fn quality_scores_mut(&mut self) -> Option<&mut Vec<u8>> {
        None
    }
    fn average_quality(&self) -> usize {
        todo!()
    }
    fn len(&self) -> usize {
        self.sequence().len()
    }
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn to_packed(&self) -> Result<Seq<Dna>> {
        let seq_bytes = self.sequence();
        let packed = Seq::trim_u8(seq_bytes)?;
        Ok(packed)
    }
}

pub trait SeqRecordExt: SeqRecord {
    // decide whether to put a sequence record on dry ice based on its average quality
    fn filter_by_score(&self, min_qual: f32) -> Option<impl SeqRecord>;
}

/// Sequence records can be bit-packed by implementing this trait.
pub trait AsBitPacked: SeqRecord {
    fn pack_seq(&self) -> Result<Seq<Dna>> {
        let seq_bytes = self.sequence();
        let packed = Seq::trim_u8(seq_bytes)?;
        Ok(packed)
    }

    fn unpack_seq(&mut self, packed_seq: Seq<Dna>);
}

// Bit-packed sequences must also be aligned into CPU words for them to be encode/for
// them to be "put on dry ice". This trait requires that you return an intermediate
// representation that implements `SafeOnDryIce`, which is actually the thing that gets
// encoded and decoded.
pub trait AsCpuWords<'a>: AsBitPacked {
    type DryIceIR: Encode + Decode<'a>;

    fn seq_to_cpu_words(&self) -> Vec<usize> {
        let bit_seq = AsBitPacked::pack_seq(self).expect("");
        let num_bases = bit_seq.len();
        bit_seq.into_raw().to_vec()
    }

    fn seq_from_cpu_words(&mut self, word: Vec<usize>);

    fn to_dry_ice(&self) -> Self::DryIceIR;

    fn from_dry_ice(dryice_ir: &Self::DryIceIR) -> Self;
}

/// Trait that should be implemented on types with sequence record behavior that can be
/// converted into a DryIce intermediate representation before encoding (and the reverse
/// for decoding)
pub trait PutOnDryIce<'a>: Encode + Decode<'a> {
    fn freeze(&self);

    /// Serialize many records to sharded files in a directory
    fn freeze_all(
        records: impl Iterator<Item = Self>,
        out_dir: impl AsRef<Path>,
        batch_size: usize,
    ) -> std::io::Result<()>;
}
pub trait TakeOffDryIce<'a>: Encode + Decode<'a> {
    fn thaw(&self);

    /// Deserialize many records from sharded files in a directory
    fn thaw_all(in_dir: impl AsRef<Path>) -> std::io::Result<Vec<Self>>;
}

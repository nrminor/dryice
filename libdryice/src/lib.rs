#![allow(dead_code, unused_variables, unused_imports)]
// #![warn(clippy::pedantic, clippy::perf)]

// module for the input and output formats that are supported
mod io {
    // standard sequence input and output formats
    mod bam;
    mod fasta;
    mod fastq;

    // opt-in support for the Apache Arrow representation as an output format;
    // gated behind the `arrow` feature.
    mod arrow;
}
pub mod errors;
pub mod prelude;

use std::{ops::Deref, path::Path};

use bincode::{Decode, Encode, de::Decoder, enc::Encoder, error};
use bio_seq::prelude::*;
use bitcode::{Decode as BitDecode, Encode as BitEncode};
use color_eyre::Result;
use log::warn;

#[derive(Debug, Encode)] // TODO: Implement decode once I've finished its manual implementation on PackedRecord
pub struct DryIceData {
    metadata: DryIceMetadata,
    records: Option<Vec<PackedRecord>>,
    // kmers: Option<Vec<Kmer<Dna, 31, usize>>>,
    // minimizers: Option<Vec<Kmer<Dna, 31, usize>>>,
}

#[derive(Debug, Encode, Decode, BitEncode, BitDecode)]
struct DryIceMetadata {
    k: Option<u8>,
    num_seqs: usize,
    source_hash: Vec<u8>,
}

#[derive(Debug)]
pub struct PackedRecord {
    id: Option<Vec<u8>>,
    seq: Seq<Dna>,
    qual: Option<Vec<u8>>,
}

/// Internal wrapper struct for a `bio-seq` bit-packed DNA sequence to make the
/// dereferencing and conversions necessary for encoding and decoding
struct PackedDna(Seq<Dna>);

impl PackedDna {
    fn from_bytes(seq_bytes: &[u8]) -> Result<Self> {
        let packed: Seq<Dna> = Seq::trim_u8(seq_bytes)?;
        Ok(Self(packed))
    }

    fn to_cpu_words(&self) -> ContiguousBitSeq {
        let num_bases = self.0.len();
        let raw_seq = self.0.into_raw().to_vec();
        ContiguousBitSeq { num_bases, raw_seq }
    }
}

#[derive(Debug, Encode, Decode, BitEncode, BitDecode)]
struct ContiguousBitSeq {
    num_bases: usize,
    raw_seq: Vec<usize>,
}

impl ContiguousBitSeq {
    fn to_bit_packed(&self) -> Result<PackedDna> {
        let maybe_seq: Option<Seq<Dna>> = Seq::from_raw(self.num_bases, &self.raw_seq);
        match maybe_seq {
            Some(seq) => Ok(PackedDna(seq)),
            None => todo!(),
        }
    }
}

impl Encode for PackedDna {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), error::EncodeError> {
        let raw_seq = self.to_cpu_words();
        bincode::Encode::encode(&raw_seq, encoder)?;
        todo!()
    }
}

impl<Context> Decode<Context> for PackedDna {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, error::DecodeError> {
        let raw_seq: ContiguousBitSeq = Decode::decode(decoder)?;
        let packed_seq = raw_seq.to_bit_packed().expect("");
        todo!()
    }
}

impl From<Seq<Dna>> for PackedDna {
    fn from(value: Seq<Dna>) -> Self {
        PackedDna(value)
    }
}

impl From<PackedDna> for ContiguousBitSeq {
    fn from(value: PackedDna) -> Self {
        value.to_cpu_words()
    }
}

impl Deref for PackedDna {
    type Target = Seq<Dna>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<Seq<Dna>> for PackedDna {
    fn as_ref(&self) -> &Seq<Dna> {
        &self.0
    }
}

impl From<PackedDna> for Seq<Dna> {
    fn from(value: PackedDna) -> Self {
        value.0
    }
}

impl AsRef<PackedDna> for Seq<Dna> {
    fn as_ref(&self) -> &PackedDna {
        todo!()
    }
}

impl Encode for PackedRecord {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> std::result::Result<(), error::EncodeError> {
        // Start with the id field, which is easy to encode without special handling
        bincode::Encode::encode(&self.id, encoder)?;

        // proceed to the sequence field, which, because it uses an external crate's data
        // structure, will take more manual handling to encode. Still not too bad though because
        // of some of Rust's standard traits.
        let packed: PackedDna = self.seq.clone().into();
        let encodable: ContiguousBitSeq = packed.to_cpu_words();
        bincode::Encode::encode(&encodable, encoder)?;

        // finish with the quality scores, which, like the id, are easy to compress because they're
        // just collections of bytes
        bincode::Encode::encode(&self.qual, encoder)?;

        Ok(())
    }
}

impl<Context> Decode<Context> for PackedRecord {
    fn decode<D: Decoder<Context = Context>>(
        decoder: &mut D,
    ) -> std::result::Result<Self, error::DecodeError> {
        let id = Decode::decode(decoder)?;
        let raw_dna: ContiguousBitSeq = Decode::decode(decoder)?;
        let packed_dna: Seq<Dna> = raw_dna.to_bit_packed().expect("").0;
        let qual = Decode::decode(decoder)?;

        let record = PackedRecord {
            id,
            seq: packed_dna,
            qual,
        };

        todo!()
    }
}

#[derive(Debug, Default)]
pub enum PhredEncoding {
    #[default]
    Phred33,
    Phred64,
}

impl PhredEncoding {
    pub fn from(val: usize) -> PhredEncoding {
        match val {
            33 => PhredEncoding::Phred33,
            64 => PhredEncoding::Phred64,
            _ => {
                warn!(
                    "Invalid encoding requested, '{}'. The default of 33 will be used.",
                    val
                );
                PhredEncoding::Phred33
            },
        }
    }
}

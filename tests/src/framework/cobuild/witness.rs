use ckb_testtool::ckb_types::{
    bytes::Bytes,
    prelude::{Builder, Entity},
};
use cobuild_types::entity::{
    core::{LockSeal, Message as CobuildMessage, OtxStart, SighashAll, SighashAllOnly},
    witness::WitnessLayout,
};

use super::otx::BuiltOtxSpec;

pub use crate::framework::tx::WitnessHandle;

pub fn lock_seal(script_hash: [u8; 32], seal: Vec<u8>) -> cobuild_types::entity::core::LockSeal {
    LockSeal::new_builder()
        .script_hash(script_hash)
        .seal(seal)
        .build()
}

#[derive(Clone, Debug)]
pub enum WitnessSpec {
    Empty,
    Legacy(Bytes),
    SighashAll {
        message: CobuildMessage,
        seal: Vec<u8>,
    },
    SighashAllOnly {
        seal: Vec<u8>,
    },
    OtxStart(OtxStartSpec),
    Otx(BuiltOtxSpec),
    RawCobuild(Bytes),
}

impl WitnessSpec {
    pub fn encode(self) -> Bytes {
        match self {
            Self::Empty => Bytes::new(),
            Self::Legacy(bytes) | Self::RawCobuild(bytes) => bytes,
            Self::SighashAll { message, seal } => encode_layout(WitnessLayout::from(
                SighashAll::new_builder()
                    .seal(seal)
                    .message(message)
                    .build(),
            )),
            Self::SighashAllOnly { seal } => encode_layout(WitnessLayout::from(
                SighashAllOnly::new_builder().seal(seal).build(),
            )),
            Self::OtxStart(spec) => spec.encode(),
            Self::Otx(spec) => encode_layout(WitnessLayout::from(spec.otx)),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OtxStartSpec {
    pub start_input_cell: u32,
    pub start_output_cell: u32,
    pub start_cell_deps: u32,
    pub start_header_deps: u32,
}

impl OtxStartSpec {
    pub fn encode(self) -> Bytes {
        encode_layout(WitnessLayout::from(
            OtxStart::new_builder()
                .start_input_cell(self.start_input_cell.to_le_bytes())
                .start_output_cell(self.start_output_cell.to_le_bytes())
                .start_cell_deps(self.start_cell_deps.to_le_bytes())
                .start_header_deps(self.start_header_deps.to_le_bytes())
                .build(),
        ))
    }
}

fn encode_layout(witness: WitnessLayout) -> Bytes {
    Bytes::copy_from_slice(witness.as_slice())
}

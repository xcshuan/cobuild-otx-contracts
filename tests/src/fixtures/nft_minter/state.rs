use ckb_testtool::ckb_types::bytes::Bytes;
use core::mem::size_of;

pub const CREATE_MINTER_TAG: u8 = 1;
pub const MINT_NFT_TAG: u8 = 2;
pub const HASH_LEN: usize = 32;
pub const TAG_LEN: usize = size_of::<u8>();
pub const U64_LEN: usize = size_of::<u64>();
pub const MINTER_DATA_LEN: usize = U64_LEN + U64_LEN;
pub const NFT_DATA_LEN: usize = HASH_LEN + U64_LEN + TAG_LEN + HASH_LEN;
pub const CREATE_ACTION_LEN: usize = TAG_LEN + U64_LEN;
pub const MINT_ACTION_LEN: usize = TAG_LEN + HASH_LEN + HASH_LEN;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MinterState {
    pub mint_counter: u64,
    pub supply_cap: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MintedNftData {
    pub minter_type_hash: [u8; 32],
    pub serial: u64,
    pub rarity: u8,
    pub attributes_hash: [u8; 32],
}

fn blake2b_256(data: impl AsRef<[u8]>) -> [u8; 32] {
    let mut out = [0u8; 32];
    let mut hasher = blake2b_ref::Blake2bBuilder::new(32).build();
    hasher.update(data.as_ref());
    hasher.finalize(&mut out);
    out
}

pub fn minter_data(state: MinterState) -> Bytes {
    let mut out = Vec::with_capacity(MINTER_DATA_LEN);
    out.extend_from_slice(&state.mint_counter.to_le_bytes());
    out.extend_from_slice(&state.supply_cap.to_le_bytes());
    out.into()
}

pub fn minted_nft_data(data: MintedNftData) -> Bytes {
    let mut out = Vec::with_capacity(NFT_DATA_LEN);
    out.extend_from_slice(&data.minter_type_hash);
    out.extend_from_slice(&data.serial.to_le_bytes());
    out.push(data.rarity);
    out.extend_from_slice(&data.attributes_hash);
    out.into()
}

pub fn create_minter_action_data(supply_cap: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(CREATE_ACTION_LEN);
    out.push(CREATE_MINTER_TAG);
    out.extend_from_slice(&supply_cap.to_le_bytes());
    out
}

pub fn mint_nft_action_data(metadata_seed: [u8; 32], mint_to_lock_hash: [u8; 32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(MINT_ACTION_LEN);
    out.push(MINT_NFT_TAG);
    out.extend_from_slice(&metadata_seed);
    out.extend_from_slice(&mint_to_lock_hash);
    out
}

pub fn rarity_for_serial(serial: u64) -> u8 {
    if serial.is_multiple_of(77) {
        3
    } else if serial.is_multiple_of(11) {
        2
    } else if serial.is_multiple_of(7) {
        1
    } else {
        0
    }
}

pub fn nft_id(minter_type_hash: [u8; 32], serial: u64) -> [u8; 32] {
    let mut input = [0u8; 40];
    input[0..32].copy_from_slice(&minter_type_hash);
    input[32..40].copy_from_slice(&serial.to_le_bytes());
    blake2b_256(input)
}

pub fn attributes_hash(
    minter_type_hash: [u8; 32],
    serial: u64,
    rarity: u8,
    metadata_seed: [u8; 32],
) -> [u8; 32] {
    let mut input = [0u8; 73];
    input[0..32].copy_from_slice(&minter_type_hash);
    input[32..40].copy_from_slice(&serial.to_le_bytes());
    input[40] = rarity;
    input[41..73].copy_from_slice(&metadata_seed);
    blake2b_256(input)
}

use crate::error::Error;

pub const MINTER_DATA_LEN: usize = 16;
pub const NFT_DATA_LEN: usize = 73;
pub const CREATE_MINTER_TAG: u8 = 1;
pub const MINT_NFT_TAG: u8 = 2;
pub const CREATE_ACTION_LEN: usize = 1 + 8;
pub const MINT_ACTION_LEN: usize = 1 + 32;

fn blake2b_256(data: impl AsRef<[u8]>) -> [u8; 32] {
    let mut out = [0u8; 32];
    let mut hasher = blake2b_ref::Blake2bBuilder::new(32).build();
    hasher.update(data.as_ref());
    hasher.finalize(&mut out);
    out
}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NftMinterAction {
    CreateMinter { supply_cap: u64 },
    MintNft { metadata_seed: [u8; 32] },
}

pub fn parse_minter_state(data: &[u8]) -> Result<MinterState, Error> {
    if data.len() != MINTER_DATA_LEN {
        return Err(Error::InvalidMinterData);
    }
    let mut counter = [0u8; 8];
    counter.copy_from_slice(&data[0..8]);
    let mut cap = [0u8; 8];
    cap.copy_from_slice(&data[8..16]);
    Ok(MinterState {
        mint_counter: u64::from_le_bytes(counter),
        supply_cap: u64::from_le_bytes(cap),
    })
}

pub fn encode_minter_state(state: MinterState) -> [u8; MINTER_DATA_LEN] {
    let mut out = [0u8; MINTER_DATA_LEN];
    out[0..8].copy_from_slice(&state.mint_counter.to_le_bytes());
    out[8..16].copy_from_slice(&state.supply_cap.to_le_bytes());
    out
}

pub fn parse_minted_nft_data(data: &[u8]) -> Result<MintedNftData, Error> {
    if data.len() != NFT_DATA_LEN {
        return Err(Error::InvalidMintedNft);
    }
    let mut minter_type_hash = [0u8; 32];
    minter_type_hash.copy_from_slice(&data[0..32]);
    let mut serial = [0u8; 8];
    serial.copy_from_slice(&data[32..40]);
    let rarity = data[40];
    let mut attributes_hash = [0u8; 32];
    attributes_hash.copy_from_slice(&data[41..73]);
    Ok(MintedNftData {
        minter_type_hash,
        serial: u64::from_le_bytes(serial),
        rarity,
        attributes_hash,
    })
}

pub fn encode_minted_nft_data(data: MintedNftData) -> [u8; NFT_DATA_LEN] {
    let mut out = [0u8; NFT_DATA_LEN];
    out[0..32].copy_from_slice(&data.minter_type_hash);
    out[32..40].copy_from_slice(&data.serial.to_le_bytes());
    out[40] = data.rarity;
    out[41..73].copy_from_slice(&data.attributes_hash);
    out
}

pub fn parse_action(data: &[u8]) -> Result<NftMinterAction, Error> {
    match data.first().copied() {
        Some(CREATE_MINTER_TAG) if data.len() == CREATE_ACTION_LEN => {
            let mut cap = [0u8; 8];
            cap.copy_from_slice(&data[1..9]);
            Ok(NftMinterAction::CreateMinter {
                supply_cap: u64::from_le_bytes(cap),
            })
        }
        Some(MINT_NFT_TAG) if data.len() == MINT_ACTION_LEN => {
            let mut metadata_seed = [0u8; 32];
            metadata_seed.copy_from_slice(&data[1..33]);
            Ok(NftMinterAction::MintNft { metadata_seed })
        }
        _ => Err(Error::InvalidAction),
    }
}

pub fn create_minter_action_data(supply_cap: u64) -> [u8; CREATE_ACTION_LEN] {
    let mut out = [0u8; CREATE_ACTION_LEN];
    out[0] = CREATE_MINTER_TAG;
    out[1..9].copy_from_slice(&supply_cap.to_le_bytes());
    out
}

pub fn mint_nft_action_data(metadata_seed: [u8; 32]) -> [u8; MINT_ACTION_LEN] {
    let mut out = [0u8; MINT_ACTION_LEN];
    out[0] = MINT_NFT_TAG;
    out[1..33].copy_from_slice(&metadata_seed);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rarity_treats_zero_as_rare3() {
        assert_eq!(rarity_for_serial(0), 3);
        assert_eq!(rarity_for_serial(6), 0);
        assert_eq!(rarity_for_serial(7), 1);
        assert_eq!(rarity_for_serial(11), 2);
        assert_eq!(rarity_for_serial(77), 3);
    }

    #[test]
    fn minter_state_round_trips() {
        let state = MinterState {
            mint_counter: 6,
            supply_cap: 100,
        };
        assert_eq!(parse_minter_state(&encode_minter_state(state)), Ok(state));
    }

    #[test]
    fn actions_parse_create_and_mint() {
        assert_eq!(
            parse_action(&create_minter_action_data(10)),
            Ok(NftMinterAction::CreateMinter { supply_cap: 10 })
        );
        assert_eq!(
            parse_action(&mint_nft_action_data([7; 32])),
            Ok(NftMinterAction::MintNft {
                metadata_seed: [7; 32]
            })
        );
    }

    #[test]
    fn nft_id_uses_plain_blake2b_256() {
        let expected = [
            0xd2, 0xcc, 0xe0, 0x19, 0x6c, 0x7a, 0xa9, 0x03, 0xe9, 0x1a, 0xe6, 0x22, 0x17, 0x5c,
            0x6d, 0x7b, 0x87, 0x2d, 0x92, 0x3a, 0x29, 0x0c, 0xe8, 0xe9, 0x0e, 0xab, 0xa2, 0xf3,
            0x9a, 0xe2, 0xe2, 0xcc,
        ];
        assert_eq!(nft_id([1; 32], 7), expected);
    }
}

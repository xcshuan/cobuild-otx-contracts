use ckb_testtool::ckb_types::bytes::Bytes;

#[derive(Clone, Debug)]
pub struct TestUdt {
    pub type_script_hash: [u8; 32],
    pub owner_lock_hash: [u8; 32],
}

#[derive(Clone, Debug)]
pub struct TestNft {
    pub type_script_hash: [u8; 32],
    pub type_id: [u8; 32],
}

pub fn udt_amount_data(amount: u128) -> Bytes {
    Bytes::from(amount.to_le_bytes().to_vec())
}

pub fn nft_data(name: &[u8], attributes: [u8; 4], created_at: u64) -> Bytes {
    let mut data = Vec::with_capacity(1 + name.len() + 4 + 8);
    data.push(name.len() as u8);
    data.extend_from_slice(name);
    data.extend_from_slice(&attributes);
    data.extend_from_slice(&created_at.to_le_bytes());
    data.into()
}

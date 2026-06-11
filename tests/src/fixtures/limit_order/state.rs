use ckb_testtool::ckb_types::bytes::Bytes;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LimitOrderState {
    pub owner_lock_hash: [u8; 32],
    pub offered_nft_type_hash: [u8; 32],
    pub requested_asset_id: [u8; 32],
    pub requested_amount: u64,
}

pub fn order_data(order: LimitOrderState) -> Bytes {
    let mut data = Vec::with_capacity(104);
    data.extend_from_slice(&order.owner_lock_hash);
    data.extend_from_slice(&order.offered_nft_type_hash);
    data.extend_from_slice(&order.requested_asset_id);
    data.extend_from_slice(&order.requested_amount.to_le_bytes());
    Bytes::from(data)
}

pub fn settlement_data(asset_id: [u8; 32], amount: u64) -> Bytes {
    let mut data = Vec::with_capacity(40);
    data.extend_from_slice(&asset_id);
    data.extend_from_slice(&amount.to_le_bytes());
    Bytes::from(data)
}

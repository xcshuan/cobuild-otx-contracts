use ckb_testtool::{
    ckb_types::{
        bytes::Bytes,
        packed::{CellInput, CellOutput, OutPoint, Script},
        prelude::*,
    },
    context::Context,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LimitOrderState {
    pub order_id: [u8; 32],
    pub owner_lock_hash: [u8; 32],
    pub offered_asset_id: [u8; 32],
    pub requested_asset_id: [u8; 32],
    pub offered_remaining: u64,
    pub min_requested_per_offered: u64,
    pub nonce: u64,
}

pub fn order_data(order: LimitOrderState) -> Vec<u8> {
    let mut data = Vec::with_capacity(152);
    data.extend_from_slice(&order.order_id);
    data.extend_from_slice(&order.owner_lock_hash);
    data.extend_from_slice(&order.offered_asset_id);
    data.extend_from_slice(&order.requested_asset_id);
    data.extend_from_slice(&order.offered_remaining.to_le_bytes());
    data.extend_from_slice(&order.min_requested_per_offered.to_le_bytes());
    data.extend_from_slice(&order.nonce.to_le_bytes());
    data
}

pub fn settlement_data(asset_id: [u8; 32], amount: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(40);
    data.extend_from_slice(&asset_id);
    data.extend_from_slice(&amount.to_le_bytes());
    data
}

#[derive(Clone, Debug)]
pub struct TestCellOutput {
    pub cell: CellOutput,
    pub data: Bytes,
}

impl TestCellOutput {
    pub fn new(cell: CellOutput, data: impl Into<Bytes>) -> Self {
        Self {
            cell,
            data: data.into(),
        }
    }
}

pub fn normal_output(lock: Script, capacity: u64) -> CellOutput {
    CellOutput::new_builder()
        .capacity(capacity)
        .lock(lock)
        .build()
}

pub fn typed_output(lock: Script, type_script: Script, capacity: u64) -> CellOutput {
    CellOutput::new_builder()
        .capacity(capacity)
        .lock(lock)
        .type_(Some(type_script).pack())
        .build()
}

pub fn live_input(context: &mut Context, output: CellOutput, data: impl Into<Bytes>) -> CellInput {
    let out_point: OutPoint = context.create_cell(output, data.into());
    CellInput::new_builder().previous_output(out_point).build()
}

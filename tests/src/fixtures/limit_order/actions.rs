use crate::framework::tx::{BuiltTxShape, OutputHandle};

use super::{LimitOrderState, order_data};

pub(crate) const CREATE_ORDER_TAG: u8 = 1;
pub(crate) const FILL_ORDER_TAG: u8 = 2;
pub(crate) const UNKNOWN_ACTION_TAG: u8 = 0xff;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitOrderAction {
    Create {
        order: LimitOrderState,
    },
    Fill {
        payment: OutputHandle,
        buyer_lock_hash: [u8; 32],
    },
    UnknownTag,
    MalformedFill {
        payment: OutputHandle,
        buyer_lock_hash: [u8; 32],
    },
}

pub fn encode_action(action: &LimitOrderAction, built: &BuiltTxShape) -> Vec<u8> {
    match *action {
        LimitOrderAction::Create { order } => create_order_action_data(order),
        LimitOrderAction::Fill {
            payment,
            buyer_lock_hash,
        } => fill_order_action_data_by_index(payment_tx_index(payment, built), buyer_lock_hash),
        LimitOrderAction::UnknownTag => {
            let mut data = Vec::with_capacity(37);
            data.push(UNKNOWN_ACTION_TAG);
            data.extend_from_slice(&[0u8; 36]);
            data
        }
        LimitOrderAction::MalformedFill {
            payment,
            buyer_lock_hash,
        } => {
            let mut data =
                fill_order_action_data_by_index(payment_tx_index(payment, built), buyer_lock_hash);
            data.pop();
            data
        }
    }
}

pub fn create_order_action_data(order: LimitOrderState) -> Vec<u8> {
    let mut data = Vec::with_capacity(105);
    data.push(CREATE_ORDER_TAG);
    data.extend_from_slice(order_data(order).as_ref());
    data
}

pub(super) fn fill_order_action_data_by_index(
    payment_output_index: u32,
    buyer_lock_hash: [u8; 32],
) -> Vec<u8> {
    let mut data = Vec::with_capacity(37);
    data.push(FILL_ORDER_TAG);
    data.extend_from_slice(&payment_output_index.to_le_bytes());
    data.extend_from_slice(&buyer_lock_hash);
    data
}

fn payment_tx_index(payment: OutputHandle, built: &BuiltTxShape) -> u32 {
    built
        .outputs
        .tx_index(payment)
        .try_into()
        .expect("payment output index fits in u32")
}

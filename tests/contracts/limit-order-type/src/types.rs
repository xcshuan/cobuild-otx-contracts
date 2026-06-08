use crate::error::Error;

pub const ORDER_DATA_LEN: usize = 152;
pub const SETTLEMENT_DATA_LEN: usize = 40;
pub const UDT_PAYMENT_DATA_LEN: usize = 16;
pub const FILL_ORDER_TAG: u8 = 1;
const FILL_ORDER_DATA_LEN: usize = 81;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderState {
    pub order_id: [u8; 32],
    pub owner_lock_hash: [u8; 32],
    pub offered_asset_id: [u8; 32],
    pub requested_asset_id: [u8; 32],
    pub offered_remaining: u64,
    pub min_requested_per_offered: u64,
    pub nonce: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementCell {
    pub owner_lock_hash: [u8; 32],
    pub asset_id: [u8; 32],
    pub amount: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FillOrderAction {
    pub order_id: [u8; 32],
    pub requested_asset_id: [u8; 32],
    pub offered_amount: u64,
    pub min_requested_amount: u64,
}

pub fn parse_order_state(data: &[u8]) -> Result<OrderState, Error> {
    if data.len() != ORDER_DATA_LEN {
        return Err(Error::InvalidOrderData);
    }

    Ok(OrderState {
        order_id: read_bytes32(data, 0),
        owner_lock_hash: read_bytes32(data, 32),
        offered_asset_id: read_bytes32(data, 64),
        requested_asset_id: read_bytes32(data, 96),
        offered_remaining: read_u64(data, 128),
        min_requested_per_offered: read_u64(data, 136),
        nonce: read_u64(data, 144),
    })
}

pub fn parse_settlement_cell(
    owner_lock_hash: [u8; 32],
    data: &[u8],
) -> Result<SettlementCell, Error> {
    if data.len() != SETTLEMENT_DATA_LEN {
        return Err(Error::InvalidSettlementData);
    }

    Ok(SettlementCell {
        owner_lock_hash,
        asset_id: read_bytes32(data, 0),
        amount: read_u64(data, 32),
    })
}

pub fn parse_udt_payment(
    owner_lock_hash: [u8; 32],
    asset_id: [u8; 32],
    data: &[u8],
) -> Result<SettlementCell, Error> {
    if data.len() != UDT_PAYMENT_DATA_LEN {
        return Err(Error::InvalidSettlementData);
    }

    Ok(SettlementCell {
        owner_lock_hash,
        asset_id,
        amount: read_u128_as_u64(data, 0)?,
    })
}

pub fn parse_fill_order_action(data: &[u8]) -> Result<FillOrderAction, Error> {
    if data.len() != FILL_ORDER_DATA_LEN {
        return Err(Error::InvalidActionData);
    }
    if data[0] != FILL_ORDER_TAG {
        return Err(Error::UnsupportedAction);
    }

    Ok(FillOrderAction {
        order_id: read_bytes32(data, 1),
        requested_asset_id: read_bytes32(data, 33),
        offered_amount: read_u64(data, 65),
        min_requested_amount: read_u64(data, 73),
    })
}

pub fn required_requested_amount(order: &OrderState) -> Result<u64, Error> {
    order
        .offered_remaining
        .checked_mul(order.min_requested_per_offered)
        .ok_or(Error::AmountOverflow)
}

pub fn validate_fill(
    order: &OrderState,
    action: &FillOrderAction,
    settlements: &[SettlementCell],
) -> Result<(), Error> {
    if action.order_id != order.order_id
        || action.requested_asset_id != order.requested_asset_id
        || action.offered_amount != order.offered_remaining
    {
        return Err(Error::ActionMismatch);
    }

    let required = required_requested_amount(order)?;
    if action.min_requested_amount < required {
        return Err(Error::InsufficientPayment);
    }

    let paid = settlements.iter().try_fold(0u64, |paid, settlement| {
        if settlement.owner_lock_hash == order.owner_lock_hash
            && settlement.asset_id == order.requested_asset_id
        {
            paid.checked_add(settlement.amount)
                .ok_or(Error::AmountOverflow)
        } else {
            Ok(paid)
        }
    })?;

    if paid < action.min_requested_amount {
        return Err(Error::InsufficientPayment);
    }

    Ok(())
}

fn read_bytes32(data: &[u8], offset: usize) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&data[offset..offset + 32]);
    out
}

fn read_u64(data: &[u8], offset: usize) -> u64 {
    let mut out = [0u8; 8];
    out.copy_from_slice(&data[offset..offset + 8]);
    u64::from_le_bytes(out)
}

fn read_u128_as_u64(data: &[u8], offset: usize) -> Result<u64, Error> {
    let mut out = [0u8; 16];
    out.copy_from_slice(&data[offset..offset + 16]);
    u64::try_from(u128::from_le_bytes(out)).map_err(|_| Error::AmountOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    const ORDER_ID: [u8; 32] = [1; 32];
    const OWNER_LOCK_HASH: [u8; 32] = [2; 32];
    const OFFERED_ASSET_ID: [u8; 32] = [3; 32];
    const REQUESTED_ASSET_ID: [u8; 32] = [4; 32];

    fn order_data(offered_remaining: u64, min_requested_per_offered: u64) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&ORDER_ID);
        data.extend_from_slice(&OWNER_LOCK_HASH);
        data.extend_from_slice(&OFFERED_ASSET_ID);
        data.extend_from_slice(&REQUESTED_ASSET_ID);
        data.extend_from_slice(&offered_remaining.to_le_bytes());
        data.extend_from_slice(&min_requested_per_offered.to_le_bytes());
        data.extend_from_slice(&9u64.to_le_bytes());
        data
    }

    fn fill_action_data(offered_amount: u64, min_requested_amount: u64) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(FILL_ORDER_TAG);
        data.extend_from_slice(&ORDER_ID);
        data.extend_from_slice(&REQUESTED_ASSET_ID);
        data.extend_from_slice(&offered_amount.to_le_bytes());
        data.extend_from_slice(&min_requested_amount.to_le_bytes());
        data
    }

    fn order_state() -> OrderState {
        parse_order_state(&order_data(10, 3)).expect("order data")
    }

    fn fill_action(min_requested_amount: u64) -> FillOrderAction {
        parse_fill_order_action(&fill_action_data(10, min_requested_amount)).expect("fill action")
    }

    fn settlement(owner_lock_hash: [u8; 32], asset_id: [u8; 32], amount: u64) -> SettlementCell {
        SettlementCell {
            owner_lock_hash,
            asset_id,
            amount,
        }
    }

    #[test]
    fn generated_proxy_lock_code_hash_is_32_bytes() {
        assert_eq!(
            crate::generated_proxy_lock::INPUT_TYPE_PROXY_LOCK_CODE_HASH.len(),
            32
        );
        assert_ne!(
            crate::generated_proxy_lock::INPUT_TYPE_PROXY_LOCK_CODE_HASH,
            [0u8; 32]
        );
    }

    #[test]
    fn parse_order_state_reads_fixed_width_fields() {
        let order = parse_order_state(&order_data(10, 3)).expect("order data");

        assert_eq!(order.order_id, ORDER_ID);
        assert_eq!(order.owner_lock_hash, OWNER_LOCK_HASH);
        assert_eq!(order.offered_asset_id, OFFERED_ASSET_ID);
        assert_eq!(order.requested_asset_id, REQUESTED_ASSET_ID);
        assert_eq!(order.offered_remaining, 10);
        assert_eq!(order.min_requested_per_offered, 3);
        assert_eq!(order.nonce, 9);
    }

    #[test]
    fn parse_order_state_rejects_truncated_data() {
        let data = order_data(10, 3);

        assert_eq!(
            parse_order_state(&data[..ORDER_DATA_LEN - 1]).unwrap_err(),
            Error::InvalidOrderData
        );
    }

    #[test]
    fn parse_fill_order_action_reads_fixed_width_fields() {
        let action = parse_fill_order_action(&fill_action_data(10, 30)).expect("fill action");

        assert_eq!(action.order_id, ORDER_ID);
        assert_eq!(action.requested_asset_id, REQUESTED_ASSET_ID);
        assert_eq!(action.offered_amount, 10);
        assert_eq!(action.min_requested_amount, 30);
    }

    #[test]
    fn parse_fill_order_action_rejects_unknown_variant() {
        let mut data = fill_action_data(10, 30);
        data[0] = 99;

        assert_eq!(
            parse_fill_order_action(&data).unwrap_err(),
            Error::UnsupportedAction
        );
    }

    #[test]
    fn parse_udt_payment_reads_16_byte_amount() {
        let payment = parse_udt_payment(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, &30u128.to_le_bytes())
            .expect("udt payment");

        assert_eq!(payment.owner_lock_hash, OWNER_LOCK_HASH);
        assert_eq!(payment.asset_id, REQUESTED_ASSET_ID);
        assert_eq!(payment.amount, 30);
    }

    #[test]
    fn parse_udt_payment_rejects_malformed_amount() {
        assert_eq!(
            parse_udt_payment(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, &[0u8; 15]),
            Err(Error::InvalidSettlementData)
        );
    }

    #[test]
    fn required_requested_amount_multiplies_remaining_by_limit_price() {
        let order = parse_order_state(&order_data(10, 3)).expect("order data");

        assert_eq!(required_requested_amount(&order), Ok(30));
    }

    #[test]
    fn required_requested_amount_rejects_overflow() {
        let order = parse_order_state(&order_data(u64::MAX, 2)).expect("order data");

        assert_eq!(
            required_requested_amount(&order),
            Err(Error::AmountOverflow)
        );
    }

    #[test]
    fn validate_fill_accepts_exact_owner_payment() {
        let settlements = [settlement(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 30)];

        assert_eq!(
            validate_fill(&order_state(), &fill_action(30), &settlements),
            Ok(())
        );
    }

    #[test]
    fn validate_fill_accepts_owner_overpayment() {
        let settlements = [settlement(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 31)];

        assert_eq!(
            validate_fill(&order_state(), &fill_action(30), &settlements),
            Ok(())
        );
    }

    #[test]
    fn validate_fill_rejects_insufficient_owner_payment() {
        let settlements = [settlement(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 29)];

        assert_eq!(
            validate_fill(&order_state(), &fill_action(30), &settlements),
            Err(Error::InsufficientPayment)
        );
    }

    #[test]
    fn validate_fill_rejects_payment_to_wrong_owner() {
        let settlements = [settlement([9; 32], REQUESTED_ASSET_ID, 30)];

        assert_eq!(
            validate_fill(&order_state(), &fill_action(30), &settlements),
            Err(Error::InsufficientPayment)
        );
    }

    #[test]
    fn validate_fill_rejects_payment_with_wrong_asset_id() {
        let settlements = [settlement(OWNER_LOCK_HASH, [9; 32], 30)];

        assert_eq!(
            validate_fill(&order_state(), &fill_action(30), &settlements),
            Err(Error::InsufficientPayment)
        );
    }

    #[test]
    fn validate_fill_rejects_action_that_does_not_match_order() {
        let mut action = fill_action(30);
        action.order_id = [9; 32];

        assert_eq!(
            validate_fill(&order_state(), &action, &[]),
            Err(Error::ActionMismatch)
        );
    }

    #[test]
    fn validate_fill_rejects_requested_asset_mismatch() {
        let mut action = fill_action(30);
        action.requested_asset_id = [9; 32];

        assert_eq!(
            validate_fill(&order_state(), &action, &[]),
            Err(Error::ActionMismatch)
        );
    }

    #[test]
    fn validate_fill_rejects_offered_amount_mismatch() {
        let mut action = fill_action(30);
        action.offered_amount = 9;

        assert_eq!(
            validate_fill(&order_state(), &action, &[]),
            Err(Error::ActionMismatch)
        );
    }

    #[test]
    fn validate_fill_rejects_action_min_below_required_even_if_paid() {
        let settlements = [settlement(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 30)];

        assert_eq!(
            validate_fill(&order_state(), &fill_action(29), &settlements),
            Err(Error::InsufficientPayment)
        );
    }

    #[test]
    fn validate_fill_rejects_payment_sum_overflow() {
        let settlements = [
            settlement(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, u64::MAX),
            settlement(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 1),
        ];

        assert_eq!(
            validate_fill(&order_state(), &fill_action(30), &settlements),
            Err(Error::AmountOverflow)
        );
    }
}

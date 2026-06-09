use crate::error::Error;
use alloc::vec::Vec;

pub const ORDER_DATA_LEN: usize = 104;
pub const SETTLEMENT_DATA_LEN: usize = 40;
pub const UDT_PAYMENT_DATA_LEN: usize = 16;
pub const CREATE_ORDER_TAG: u8 = 1;
pub const FILL_ORDER_TAG: u8 = 2;
const CREATE_ORDER_DATA_LEN: usize = 105;
const FILL_ORDER_DATA_LEN: usize = 45;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderState {
    pub owner_lock_hash: [u8; 32],
    pub offered_nft_type_hash: [u8; 32],
    pub requested_asset_id: [u8; 32],
    pub min_requested_amount: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementCell {
    pub owner_lock_hash: [u8; 32],
    pub asset_id: [u8; 32],
    pub amount: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateOrderAction {
    pub owner_lock_hash: [u8; 32],
    pub offered_nft_type_hash: [u8; 32],
    pub requested_asset_id: [u8; 32],
    pub min_requested_amount: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FillOrderAction {
    pub requested_asset_id: [u8; 32],
    pub min_requested_amount: u64,
    pub payment_output_index: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitOrderAction {
    Create(CreateOrderAction),
    Fill(FillOrderAction),
}

pub trait BoundPayment {
    fn bound_payment(self) -> Result<SettlementCell, Error>;
}

impl BoundPayment for SettlementCell {
    fn bound_payment(self) -> Result<SettlementCell, Error> {
        Ok(self)
    }
}

impl BoundPayment for &[SettlementCell] {
    fn bound_payment(self) -> Result<SettlementCell, Error> {
        match self {
            [payment] => Ok(*payment),
            _ => Err(Error::InsufficientPayment),
        }
    }
}

impl BoundPayment for &Vec<SettlementCell> {
    fn bound_payment(self) -> Result<SettlementCell, Error> {
        self.as_slice().bound_payment()
    }
}

pub fn parse_order_state(data: &[u8]) -> Result<OrderState, Error> {
    if data.len() != ORDER_DATA_LEN {
        return Err(Error::InvalidOrderData);
    }

    Ok(OrderState {
        owner_lock_hash: read_bytes32(data, 0),
        offered_nft_type_hash: read_bytes32(data, 32),
        requested_asset_id: read_bytes32(data, 64),
        min_requested_amount: read_u64(data, 96),
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

pub fn parse_limit_order_action(data: &[u8]) -> Result<LimitOrderAction, Error> {
    let Some((&tag, _)) = data.split_first() else {
        return Err(Error::InvalidActionData);
    };

    match tag {
        CREATE_ORDER_TAG => parse_create_order_action(data).map(LimitOrderAction::Create),
        FILL_ORDER_TAG => parse_fill_order_action(data).map(LimitOrderAction::Fill),
        _ => Err(Error::UnsupportedAction),
    }
}

pub fn parse_create_order_action(data: &[u8]) -> Result<CreateOrderAction, Error> {
    if data.len() != CREATE_ORDER_DATA_LEN {
        return Err(Error::InvalidActionData);
    }

    Ok(CreateOrderAction {
        owner_lock_hash: read_bytes32(data, 1),
        offered_nft_type_hash: read_bytes32(data, 33),
        requested_asset_id: read_bytes32(data, 65),
        min_requested_amount: read_u64(data, 97),
    })
}

pub fn parse_fill_order_action(data: &[u8]) -> Result<FillOrderAction, Error> {
    if data.len() != FILL_ORDER_DATA_LEN {
        return Err(Error::InvalidActionData);
    }

    Ok(FillOrderAction {
        requested_asset_id: read_bytes32(data, 1),
        min_requested_amount: read_u64(data, 33),
        payment_output_index: read_u32(data, 41),
    })
}

pub fn validate_create(order: &OrderState, action: &CreateOrderAction) -> Result<(), Error> {
    if order.owner_lock_hash != action.owner_lock_hash
        || order.offered_nft_type_hash != action.offered_nft_type_hash
        || order.requested_asset_id != action.requested_asset_id
        || order.min_requested_amount != action.min_requested_amount
    {
        return Err(Error::ActionMismatch);
    }
    Ok(())
}

pub fn validate_fill(
    order: &OrderState,
    action: &FillOrderAction,
    payment: impl BoundPayment,
) -> Result<(), Error> {
    if action.requested_asset_id != order.requested_asset_id {
        return Err(Error::ActionMismatch);
    }

    if action.min_requested_amount < order.min_requested_amount {
        return Err(Error::InsufficientPayment);
    }

    let payment = payment.bound_payment()?;
    if payment.owner_lock_hash != order.owner_lock_hash
        || payment.asset_id != order.requested_asset_id
        || payment.amount < action.min_requested_amount
    {
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

fn read_u32(data: &[u8], offset: usize) -> u32 {
    let mut out = [0u8; 4];
    out.copy_from_slice(&data[offset..offset + 4]);
    u32::from_le_bytes(out)
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

    const OWNER_LOCK_HASH: [u8; 32] = [2; 32];
    const OFFERED_ASSET_ID: [u8; 32] = [3; 32];
    const REQUESTED_ASSET_ID: [u8; 32] = [4; 32];

    fn order_data(min_requested_amount: u64) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&OWNER_LOCK_HASH);
        data.extend_from_slice(&OFFERED_ASSET_ID);
        data.extend_from_slice(&REQUESTED_ASSET_ID);
        data.extend_from_slice(&min_requested_amount.to_le_bytes());
        data
    }

    fn create_action_data(min_requested_amount: u64) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(CREATE_ORDER_TAG);
        data.extend_from_slice(&OWNER_LOCK_HASH);
        data.extend_from_slice(&OFFERED_ASSET_ID);
        data.extend_from_slice(&REQUESTED_ASSET_ID);
        data.extend_from_slice(&min_requested_amount.to_le_bytes());
        data
    }

    fn fill_action_data(min_requested_amount: u64, payment_output_index: u32) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(FILL_ORDER_TAG);
        data.extend_from_slice(&REQUESTED_ASSET_ID);
        data.extend_from_slice(&min_requested_amount.to_le_bytes());
        data.extend_from_slice(&payment_output_index.to_le_bytes());
        data
    }

    fn order_state(min_requested_amount: u64) -> OrderState {
        parse_order_state(&order_data(min_requested_amount)).expect("order data")
    }

    fn create_action(min_requested_amount: u64) -> CreateOrderAction {
        parse_create_order_action(&create_action_data(min_requested_amount)).expect("create action")
    }

    fn fill_action(min_requested_amount: u64, payment_output_index: u32) -> FillOrderAction {
        parse_fill_order_action(&fill_action_data(
            min_requested_amount,
            payment_output_index,
        ))
        .expect("fill action")
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
    fn parse_order_state_reads_nft_order_fields() {
        let order = parse_order_state(&order_data(30)).expect("order data");

        assert_eq!(order.owner_lock_hash, OWNER_LOCK_HASH);
        assert_eq!(order.offered_nft_type_hash, OFFERED_ASSET_ID);
        assert_eq!(order.requested_asset_id, REQUESTED_ASSET_ID);
        assert_eq!(order.min_requested_amount, 30);
    }

    #[test]
    fn parse_order_state_rejects_truncated_data() {
        let data = order_data(30);

        assert_eq!(
            parse_order_state(&data[..ORDER_DATA_LEN - 1]).unwrap_err(),
            Error::InvalidOrderData
        );
    }

    #[test]
    fn parse_create_order_action_reads_state_payload() {
        let action = parse_limit_order_action(&create_action_data(30)).expect("create action");

        assert_eq!(
            action,
            LimitOrderAction::Create(CreateOrderAction {
                owner_lock_hash: OWNER_LOCK_HASH,
                offered_nft_type_hash: OFFERED_ASSET_ID,
                requested_asset_id: REQUESTED_ASSET_ID,
                min_requested_amount: 30,
            })
        );
    }

    #[test]
    fn parse_fill_order_action_reads_requested_asset_and_amount() {
        let action = parse_limit_order_action(&fill_action_data(30, 1)).expect("fill action");

        assert_eq!(
            action,
            LimitOrderAction::Fill(FillOrderAction {
                requested_asset_id: REQUESTED_ASSET_ID,
                min_requested_amount: 30,
                payment_output_index: 1,
            })
        );
    }

    #[test]
    fn parse_fill_order_action_reads_payment_output_index_little_endian() {
        let action = parse_limit_order_action(&fill_action_data(30, 0x0403_0201))
            .expect("fill action");

        assert_eq!(
            action,
            LimitOrderAction::Fill(FillOrderAction {
                requested_asset_id: REQUESTED_ASSET_ID,
                min_requested_amount: 30,
                payment_output_index: 0x0403_0201,
            })
        );
    }

    #[test]
    fn parse_fill_order_action_rejects_legacy_41_byte_payload() {
        let mut data = Vec::new();
        data.push(FILL_ORDER_TAG);
        data.extend_from_slice(&REQUESTED_ASSET_ID);
        data.extend_from_slice(&30u64.to_le_bytes());

        assert_eq!(
            parse_limit_order_action(&data).unwrap_err(),
            Error::InvalidActionData
        );
    }

    #[test]
    fn parse_fill_order_action_rejects_unknown_variant() {
        let mut data = fill_action_data(30, 1);
        data[0] = 99;

        assert_eq!(
            parse_limit_order_action(&data).unwrap_err(),
            Error::UnsupportedAction
        );
    }

    #[test]
    fn validate_create_accepts_matching_state() {
        let order = order_state(30);
        let action = create_action(30);

        assert_eq!(validate_create(&order, &action), Ok(()));
    }

    #[test]
    fn validate_create_rejects_state_mismatch() {
        let order = order_state(30);
        let mut action = create_action(30);
        action.min_requested_amount = 31;

        assert_eq!(validate_create(&order, &action), Err(Error::ActionMismatch));
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
    fn validate_fill_accepts_bound_payment() {
        let payment = settlement(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 30);

        assert_eq!(
            validate_fill(&order_state(30), &fill_action(30, 1), payment),
            Ok(())
        );
    }

    #[test]
    fn validate_fill_accepts_owner_overpayment() {
        let payment = settlement(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 31);

        assert_eq!(
            validate_fill(&order_state(30), &fill_action(30, 1), payment),
            Ok(())
        );
    }

    #[test]
    fn validate_fill_rejects_insufficient_owner_payment() {
        let payment = settlement(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 29);

        assert_eq!(
            validate_fill(&order_state(30), &fill_action(30, 1), payment),
            Err(Error::InsufficientPayment)
        );
    }

    #[test]
    fn validate_fill_rejects_bound_payment_to_wrong_owner() {
        let payment = settlement([9; 32], REQUESTED_ASSET_ID, 30);

        assert_eq!(
            validate_fill(&order_state(30), &fill_action(30, 1), payment),
            Err(Error::InsufficientPayment)
        );
    }

    #[test]
    fn validate_fill_rejects_bound_payment_with_wrong_asset_id() {
        let payment = settlement(OWNER_LOCK_HASH, [9; 32], 30);

        assert_eq!(
            validate_fill(&order_state(30), &fill_action(30, 1), payment),
            Err(Error::InsufficientPayment)
        );
    }

    #[test]
    fn validate_fill_rejects_action_min_below_order_minimum() {
        let mut action = fill_action(30, 1);
        action.min_requested_amount = 29;

        assert_eq!(
            validate_fill(
                &order_state(30),
                &action,
                settlement(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 30),
            ),
            Err(Error::InsufficientPayment)
        );
    }

    #[test]
    fn validate_fill_rejects_requested_asset_mismatch() {
        let mut action = fill_action(30, 1);
        action.requested_asset_id = [9; 32];

        assert_eq!(
            validate_fill(
                &order_state(30),
                &action,
                settlement(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 30),
            ),
            Err(Error::ActionMismatch)
        );
    }

    #[test]
    fn validate_fill_rejects_action_min_below_required_even_if_paid() {
        let payment = settlement(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 30);

        assert_eq!(
            validate_fill(&order_state(30), &fill_action(29, 1), payment),
            Err(Error::InsufficientPayment)
        );
    }
}

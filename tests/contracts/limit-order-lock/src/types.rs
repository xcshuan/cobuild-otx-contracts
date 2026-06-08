use crate::error::Error;

pub const ORDER_ARGS_LEN: usize = 104;
pub const UDT_PAYMENT_DATA_LEN: usize = 16;
pub const FILL_ORDER_TAG: u8 = 2;
pub const FILL_ORDER_DATA_LEN: usize = 41;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderArgs {
    pub owner_lock_hash: [u8; 32],
    pub offered_nft_type_hash: [u8; 32],
    pub requested_asset_id: [u8; 32],
    pub min_requested_amount: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FillOrderAction {
    pub requested_asset_id: [u8; 32],
    pub min_requested_amount: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UdtPayment {
    pub owner_lock_hash: [u8; 32],
    pub asset_id: [u8; 32],
    pub amount: u64,
}

pub fn parse_order_args(data: &[u8]) -> Result<OrderArgs, Error> {
    if data.len() != ORDER_ARGS_LEN {
        return Err(Error::InvalidArgs);
    }

    Ok(OrderArgs {
        owner_lock_hash: read_bytes32(data, 0),
        offered_nft_type_hash: read_bytes32(data, 32),
        requested_asset_id: read_bytes32(data, 64),
        min_requested_amount: read_u64(data, 96),
    })
}

pub fn parse_fill_order_action(data: &[u8]) -> Result<FillOrderAction, Error> {
    let Some((&tag, _)) = data.split_first() else {
        return Err(Error::InvalidActionData);
    };
    if tag != FILL_ORDER_TAG {
        return Err(Error::UnsupportedAction);
    }
    if data.len() != FILL_ORDER_DATA_LEN {
        return Err(Error::InvalidActionData);
    }

    Ok(FillOrderAction {
        requested_asset_id: read_bytes32(data, 1),
        min_requested_amount: read_u64(data, 33),
    })
}

pub fn parse_udt_payment(data: &[u8]) -> Result<u64, Error> {
    if data.len() != UDT_PAYMENT_DATA_LEN {
        return Err(Error::InvalidActionData);
    }

    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(data);
    u64::try_from(u128::from_le_bytes(bytes)).map_err(|_| Error::AmountOverflow)
}

pub fn validate_fill(
    order: &OrderArgs,
    action: &FillOrderAction,
    payments: &[UdtPayment],
) -> Result<(), Error> {
    if action.requested_asset_id != order.requested_asset_id {
        return Err(Error::ActionMismatch);
    }
    if action.min_requested_amount < order.min_requested_amount {
        return Err(Error::InsufficientPayment);
    }

    let paid = payments.iter().try_fold(0u64, |paid, payment| {
        if payment.owner_lock_hash == order.owner_lock_hash
            && payment.asset_id == order.requested_asset_id
        {
            paid.checked_add(payment.amount)
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    const OWNER_LOCK_HASH: [u8; 32] = [2; 32];
    const NFT_TYPE_HASH: [u8; 32] = [3; 32];
    const REQUESTED_ASSET_ID: [u8; 32] = [4; 32];

    fn order_args(min_requested_amount: u64) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&OWNER_LOCK_HASH);
        data.extend_from_slice(&NFT_TYPE_HASH);
        data.extend_from_slice(&REQUESTED_ASSET_ID);
        data.extend_from_slice(&min_requested_amount.to_le_bytes());
        data
    }

    fn fill_action_data(asset_id: [u8; 32], min_requested_amount: u64) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(FILL_ORDER_TAG);
        data.extend_from_slice(&asset_id);
        data.extend_from_slice(&min_requested_amount.to_le_bytes());
        data
    }

    #[test]
    fn parse_order_args_reads_fixed_width_fields() {
        let args = parse_order_args(&order_args(30)).expect("order args");

        assert_eq!(args.owner_lock_hash, OWNER_LOCK_HASH);
        assert_eq!(args.offered_nft_type_hash, NFT_TYPE_HASH);
        assert_eq!(args.requested_asset_id, REQUESTED_ASSET_ID);
        assert_eq!(args.min_requested_amount, 30);
    }

    #[test]
    fn parse_order_args_rejects_short_and_long_data() {
        let mut short = order_args(30);
        short.pop();
        let mut long = order_args(30);
        long.push(0);

        assert_eq!(parse_order_args(&short), Err(Error::InvalidArgs));
        assert_eq!(parse_order_args(&long), Err(Error::InvalidArgs));
    }

    #[test]
    fn parse_fill_action_accepts_tag_two() {
        let action = parse_fill_order_action(&fill_action_data(REQUESTED_ASSET_ID, 30))
            .expect("fill action");

        assert_eq!(action.requested_asset_id, REQUESTED_ASSET_ID);
        assert_eq!(action.min_requested_amount, 30);
    }

    #[test]
    fn parse_fill_action_rejects_unknown_tag_and_bad_lengths() {
        assert_eq!(parse_fill_order_action(&[]), Err(Error::InvalidActionData));

        let mut unknown = fill_action_data(REQUESTED_ASSET_ID, 30);
        unknown[0] = 1;
        assert_eq!(
            parse_fill_order_action(&unknown),
            Err(Error::UnsupportedAction)
        );

        let mut short = fill_action_data(REQUESTED_ASSET_ID, 30);
        short.pop();
        let mut long = fill_action_data(REQUESTED_ASSET_ID, 30);
        long.push(0);
        assert_eq!(
            parse_fill_order_action(&short),
            Err(Error::InvalidActionData)
        );
        assert_eq!(
            parse_fill_order_action(&long),
            Err(Error::InvalidActionData)
        );
    }

    #[test]
    fn parse_udt_payment_accepts_u64_compatible_u128() {
        assert_eq!(parse_udt_payment(&30u128.to_le_bytes()), Ok(30));
    }

    #[test]
    fn parse_udt_payment_rejects_bad_length_and_overflow() {
        assert_eq!(parse_udt_payment(&[0u8; 15]), Err(Error::InvalidActionData));
        assert_eq!(
            parse_udt_payment(&(u128::from(u64::MAX) + 1).to_le_bytes()),
            Err(Error::AmountOverflow)
        );
    }

    fn order(min_requested_amount: u64) -> OrderArgs {
        parse_order_args(&order_args(min_requested_amount)).expect("order args")
    }

    fn action(asset_id: [u8; 32], min_requested_amount: u64) -> FillOrderAction {
        parse_fill_order_action(&fill_action_data(asset_id, min_requested_amount))
            .expect("fill action")
    }

    fn payment(owner_lock_hash: [u8; 32], asset_id: [u8; 32], amount: u64) -> UdtPayment {
        UdtPayment {
            owner_lock_hash,
            asset_id,
            amount,
        }
    }

    #[test]
    fn validate_fill_accepts_exact_and_over_payment() {
        assert_eq!(
            validate_fill(
                &order(30),
                &action(REQUESTED_ASSET_ID, 30),
                &[payment(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 30)]
            ),
            Ok(())
        );

        assert_eq!(
            validate_fill(
                &order(30),
                &action(REQUESTED_ASSET_ID, 31),
                &[payment(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 40)]
            ),
            Ok(())
        );
    }

    #[test]
    fn validate_fill_rejects_action_mismatch_and_amount_below_order_minimum() {
        assert_eq!(
            validate_fill(&order(30), &action([9; 32], 30), &[]),
            Err(Error::ActionMismatch)
        );
        assert_eq!(
            validate_fill(&order(30), &action(REQUESTED_ASSET_ID, 29), &[]),
            Err(Error::InsufficientPayment)
        );
    }

    #[test]
    fn validate_fill_counts_only_matching_owner_and_asset() {
        assert_eq!(
            validate_fill(
                &order(30),
                &action(REQUESTED_ASSET_ID, 30),
                &[
                    payment([9; 32], REQUESTED_ASSET_ID, 30),
                    payment(OWNER_LOCK_HASH, [8; 32], 30),
                    payment(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 29),
                ],
            ),
            Err(Error::InsufficientPayment)
        );
    }

    #[test]
    fn validate_fill_detects_payment_sum_overflow() {
        assert_eq!(
            validate_fill(
                &order(30),
                &action(REQUESTED_ASSET_ID, 30),
                &[
                    payment(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, u64::MAX),
                    payment(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 1),
                ],
            ),
            Err(Error::AmountOverflow)
        );
    }
}

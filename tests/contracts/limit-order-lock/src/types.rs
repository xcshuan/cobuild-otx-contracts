use crate::error::Error;

pub const ORDER_ARGS_LEN: usize = 104;
pub const UDT_PAYMENT_DATA_LEN: usize = 16;
pub const FILL_ORDER_TAG: u8 = 2;
pub const FILL_ORDER_DATA_LEN: usize = 37;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderArgs {
    pub owner_lock_hash: [u8; 32],
    pub offered_nft_type_hash: [u8; 32],
    pub requested_asset_id: [u8; 32],
    pub requested_amount: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FillOrderAction {
    pub payment_output_index: u32,
    pub buyer_lock_hash: [u8; 32],
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
        requested_amount: read_u64(data, 96),
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
        payment_output_index: read_u32(data, 1),
        buyer_lock_hash: read_bytes32(data, 5),
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

pub fn validate_fill(order: &OrderArgs, payment: UdtPayment) -> Result<(), Error> {
    if payment.owner_lock_hash != order.owner_lock_hash
        || payment.asset_id != order.requested_asset_id
        || payment.amount < order.requested_amount
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    const OWNER_LOCK_HASH: [u8; 32] = [2; 32];
    const NFT_TYPE_HASH: [u8; 32] = [3; 32];
    const REQUESTED_ASSET_ID: [u8; 32] = [4; 32];

    fn order_args(requested_amount: u64) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&OWNER_LOCK_HASH);
        data.extend_from_slice(&NFT_TYPE_HASH);
        data.extend_from_slice(&REQUESTED_ASSET_ID);
        data.extend_from_slice(&requested_amount.to_le_bytes());
        data
    }

    fn fill_action_data(payment_output_index: u32, buyer_lock_hash: [u8; 32]) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(FILL_ORDER_TAG);
        data.extend_from_slice(&payment_output_index.to_le_bytes());
        data.extend_from_slice(&buyer_lock_hash);
        data
    }

    #[test]
    fn parse_order_args_reads_requested_amount() {
        let args = parse_order_args(&order_args(30)).expect("order args");

        assert_eq!(args.owner_lock_hash, OWNER_LOCK_HASH);
        assert_eq!(args.offered_nft_type_hash, NFT_TYPE_HASH);
        assert_eq!(args.requested_asset_id, REQUESTED_ASSET_ID);
        assert_eq!(args.requested_amount, 30);
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
    fn parse_fill_action_accepts_payment_index_and_buyer_lock_hash() {
        let action =
            parse_fill_order_action(&fill_action_data(0x0403_0201, [7; 32])).expect("fill action");

        assert_eq!(action.payment_output_index, 0x0403_0201);
        assert_eq!(action.buyer_lock_hash, [7; 32]);
    }

    #[test]
    fn parse_fill_action_rejects_old_41_and_45_byte_payloads() {
        assert_eq!(
            parse_fill_order_action(&legacy_41_byte_fill_action_data()),
            Err(Error::InvalidActionData)
        );
        assert_eq!(
            parse_fill_order_action(&legacy_45_byte_fill_action_data()),
            Err(Error::InvalidActionData)
        );
    }

    #[test]
    fn parse_fill_action_rejects_unknown_tag_and_bad_lengths() {
        assert_eq!(parse_fill_order_action(&[]), Err(Error::InvalidActionData));

        let mut unknown = fill_action_data(1, [7; 32]);
        unknown[0] = 1;
        assert_eq!(
            parse_fill_order_action(&unknown),
            Err(Error::UnsupportedAction)
        );

        let mut short = fill_action_data(1, [7; 32]);
        short.pop();
        let mut long = fill_action_data(1, [7; 32]);
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

    fn order(requested_amount: u64) -> OrderArgs {
        parse_order_args(&order_args(requested_amount)).expect("order args")
    }

    fn payment(owner_lock_hash: [u8; 32], asset_id: [u8; 32], amount: u64) -> UdtPayment {
        UdtPayment {
            owner_lock_hash,
            asset_id,
            amount,
        }
    }

    #[test]
    fn validate_fill_uses_order_requested_amount() {
        assert_eq!(
            validate_fill(&order(30), payment(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 30),),
            Ok(())
        );

        assert_eq!(
            validate_fill(&order(30), payment(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 40),),
            Ok(())
        );
        assert_eq!(
            validate_fill(&order(30), payment(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 29),),
            Err(Error::InsufficientPayment)
        );
    }

    #[test]
    fn validate_fill_accepts_bound_payment() {
        assert_eq!(
            validate_fill(&order(30), payment(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 30),),
            Ok(())
        );
    }

    #[test]
    fn validate_fill_rejects_bound_payment_wrong_owner_or_asset() {
        assert_eq!(
            validate_fill(&order(30), payment([9; 32], REQUESTED_ASSET_ID, 30),),
            Err(Error::InsufficientPayment)
        );
        assert_eq!(
            validate_fill(&order(30), payment(OWNER_LOCK_HASH, [9; 32], 30),),
            Err(Error::InsufficientPayment)
        );
    }

    fn legacy_41_byte_fill_action_data() -> Vec<u8> {
        let mut data = Vec::new();
        data.push(FILL_ORDER_TAG);
        data.extend_from_slice(&REQUESTED_ASSET_ID);
        data.extend_from_slice(&30u64.to_le_bytes());
        data
    }

    fn legacy_45_byte_fill_action_data() -> Vec<u8> {
        let mut data = legacy_41_byte_fill_action_data();
        data.extend_from_slice(&1u32.to_le_bytes());
        data
    }
}

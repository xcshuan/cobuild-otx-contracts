use ckb_std::{
    ckb_constants::Source,
    ckb_types::{bytes::Bytes, packed::Script, prelude::*},
    high_level::{QueryIter, load_cell_data, load_cell_lock_hash, load_cell_type_hash},
};
use cobuild_core::plan::OtxMessageLayout;

use crate::{
    error::Error,
    types::{SettlementCell, parse_udt_payment},
};

pub fn ensure_create_nft_proxy_output(
    current_type_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
) -> Result<(), Error> {
    let proxy_lock_hash = expected_proxy_lock_hash(current_type_hash);
    if !has_nft_proxy_output(offered_nft_type_hash, proxy_lock_hash)? {
        return Err(Error::InvalidCobuild);
    }
    Ok(())
}

pub fn load_bound_payment(
    layout: OtxMessageLayout,
    payment_output_index: u32,
) -> Result<SettlementCell, Error> {
    let index = payment_output_index as usize;
    if !crate::otx::output_index_in_otx_outputs(layout, index)? {
        return Err(Error::InvalidCobuild);
    }
    load_udt_payment_output(index)
}

pub fn ensure_nft_delivered_to_buyer(
    layout: OtxMessageLayout,
    buyer_lock_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
) -> Result<(), Error> {
    if !has_nft_delivery_output(layout, buyer_lock_hash, offered_nft_type_hash)? {
        return Err(Error::InvalidCobuild);
    }
    Ok(())
}

fn expected_proxy_lock_hash(order_type_hash: [u8; 32]) -> [u8; 32] {
    let script = Script::new_builder()
        .code_hash(crate::generated_proxy_lock::INPUT_TYPE_PROXY_LOCK_CODE_HASH.pack())
        .hash_type(ckb_std::ckb_types::packed::Byte::new(4))
        .args(Bytes::copy_from_slice(&order_type_hash).pack())
        .build();
    script.calc_script_hash().unpack()
}

fn has_nft_proxy_output(
    offered_nft_type_hash: [u8; 32],
    proxy_lock_hash: [u8; 32],
) -> Result<bool, Error> {
    let output_count = QueryIter::new(load_cell_data, Source::Output).count();
    for index in 0..output_count {
        let lock_hash = load_cell_lock_hash(index, Source::Output)?;
        if lock_hash != proxy_lock_hash {
            continue;
        }
        let Some(type_hash) = load_cell_type_hash(index, Source::Output)? else {
            continue;
        };
        if type_hash == offered_nft_type_hash {
            return Ok(true);
        }
    }
    Ok(false)
}

fn load_udt_payment_output(index: usize) -> Result<SettlementCell, Error> {
    let data = load_cell_data(index, Source::Output)?;
    let lock_hash = load_cell_lock_hash(index, Source::Output)?;
    let Some(type_hash) = load_cell_type_hash(index, Source::Output)? else {
        return Err(Error::InsufficientPayment);
    };
    parse_udt_payment(lock_hash, type_hash, &data)
}

fn has_nft_delivery_output(
    layout: OtxMessageLayout,
    buyer_lock_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
) -> Result<bool, Error> {
    for range in [layout.base_outputs, layout.append_outputs] {
        let end = range
            .start
            .checked_add(range.count)
            .ok_or(Error::InvalidCobuild)?;
        for index in range.start..end {
            let lock_hash = load_cell_lock_hash(index, Source::Output)?;
            let type_hash = load_cell_type_hash(index, Source::Output)?;
            if nft_delivery_matches(lock_hash, type_hash, buyer_lock_hash, offered_nft_type_hash) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn nft_delivery_matches(
    lock_hash: [u8; 32],
    type_hash: Option<[u8; 32]>,
    buyer_lock_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
) -> bool {
    lock_hash == buyer_lock_hash && type_hash == Some(offered_nft_type_hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nft_delivery_match_accepts_buyer_lock_and_offered_nft_type() {
        assert!(nft_delivery_matches(
            [7; 32],
            Some([8; 32]),
            [7; 32],
            [8; 32]
        ));
    }

    #[test]
    fn nft_delivery_match_rejects_wrong_buyer_lock() {
        assert!(!nft_delivery_matches(
            [6; 32],
            Some([8; 32]),
            [7; 32],
            [8; 32]
        ));
    }

    #[test]
    fn nft_delivery_match_rejects_wrong_or_missing_nft_type() {
        assert!(!nft_delivery_matches(
            [7; 32],
            Some([9; 32]),
            [7; 32],
            [8; 32]
        ));
        assert!(!nft_delivery_matches([7; 32], None, [7; 32], [8; 32]));
    }

    #[test]
    fn expected_proxy_lock_hash_changes_with_order_type_hash() {
        let first = expected_proxy_lock_hash([1; 32]);
        let second = expected_proxy_lock_hash([2; 32]);

        assert_ne!(first, second);
    }
}

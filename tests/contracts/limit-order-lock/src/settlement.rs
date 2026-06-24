use ckb_std::{
    ckb_constants::Source,
    high_level::{load_cell_data, load_cell_lock_hash, load_cell_type_hash},
};
use cobuild_core::plan::OtxMessageLayout;

use crate::{
    error::Error,
    types::{UdtPayment, parse_udt_payment},
};

pub fn load_bound_payment(
    layout: OtxMessageLayout,
    payment_output_index: u32,
) -> Result<UdtPayment, Error> {
    let index = resolve_output_index(layout, payment_output_index as usize)?;
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

fn load_udt_payment_output(index: usize) -> Result<UdtPayment, Error> {
    let Some(asset_id) = load_cell_type_hash(index, Source::Output)? else {
        return Err(Error::InsufficientPayment);
    };
    let owner_lock_hash = load_cell_lock_hash(index, Source::Output)?;
    let data = load_cell_data(index, Source::Output)?;

    Ok(UdtPayment {
        owner_lock_hash,
        asset_id,
        amount: parse_udt_payment(&data)?,
    })
}

fn resolve_output_index(
    layout: OtxMessageLayout,
    relative_output_index: usize,
) -> Result<usize, Error> {
    let output_indexes = layout.output_indexes();
    let index = output_indexes.start + relative_output_index;
    if index < output_indexes.end {
        Ok(index)
    } else {
        Err(Error::InvalidCobuild)
    }
}

fn has_nft_delivery_output(
    layout: OtxMessageLayout,
    buyer_lock_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
) -> Result<bool, Error> {
    for index in layout.output_indexes() {
        let lock_hash = load_cell_lock_hash(index, Source::Output)?;
        let type_hash = load_cell_type_hash(index, Source::Output)?;
        if nft_delivery_matches(lock_hash, type_hash, buyer_lock_hash, offered_nft_type_hash) {
            return Ok(true);
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
    fn resolve_output_index_maps_otx_relative_output_index() {
        let layout = OtxMessageLayout {
            base_inputs: cobuild_core::layout::Range { start: 0, count: 1 },
            append_inputs: cobuild_core::layout::Range { start: 1, count: 0 },
            base_outputs: cobuild_core::layout::Range { start: 4, count: 2 },
            append_outputs: cobuild_core::layout::Range { start: 6, count: 2 },
            base_cell_deps: cobuild_core::layout::Range { start: 0, count: 0 },
            append_cell_deps: cobuild_core::layout::Range { start: 0, count: 0 },
            base_header_deps: cobuild_core::layout::Range { start: 0, count: 0 },
            append_header_deps: cobuild_core::layout::Range { start: 0, count: 0 },
        };

        assert_eq!(resolve_output_index(layout, 0), Ok(4));
        assert_eq!(resolve_output_index(layout, 1), Ok(5));
        assert_eq!(resolve_output_index(layout, 2), Ok(6));
        assert_eq!(resolve_output_index(layout, 3), Ok(7));
        assert_eq!(resolve_output_index(layout, 4), Err(Error::InvalidCobuild));
    }
}

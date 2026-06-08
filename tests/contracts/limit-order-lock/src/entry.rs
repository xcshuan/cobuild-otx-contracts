use alloc::vec::Vec;

use ckb_std::{
    ckb_constants::Source,
    ckb_types::{bytes::Bytes, prelude::*},
    high_level::{
        QueryIter, load_cell_data, load_cell_lock_hash, load_cell_type_hash, load_script,
        load_script_hash,
    },
};
use cobuild_core::{
    context::CurrentScript,
    engine::CobuildContext,
    layout::Range,
    plan::{ActionOrigin, OtxMessageLayout},
    reader::cursor_bytes,
};

use crate::{
    error::Error,
    types::{
        OrderArgs, UDT_PAYMENT_DATA_LEN, UdtPayment, parse_fill_order_action, parse_order_args,
        parse_udt_payment, validate_fill,
    },
};

pub fn main() -> Result<(), Error> {
    let script = load_script()?;
    let args: Bytes = script.args().unpack();
    let order = parse_order_args(&args)?;

    let current_lock_hash = load_script_hash()?;
    let input_index = single_group_input_index(current_lock_hash)?;
    verify_offered_nft_input(input_index, order.offered_nft_type_hash)?;

    let plan = CobuildContext::build(CurrentScript::InputLock(current_lock_hash))?
        .plan_lock_validation()?;
    if plan.related_actions.len() != 1 {
        return Err(Error::InvalidCobuild);
    }

    let related = &plan.related_actions[0];
    let layout = otx_fill_layout(&related.origin, input_index)?;
    let action_data = cursor_bytes(&related.action.data)?;
    let action = parse_fill_order_action(&action_data)?;
    let payments = collect_payments(&order, layout)?;

    validate_fill(&order, &action, &payments)
}

fn single_group_input_index(current_lock_hash: [u8; 32]) -> Result<usize, Error> {
    let group_count = QueryIter::new(load_cell_data, Source::GroupInput).count();
    if group_count != 1 {
        return Err(Error::InvalidNftInput);
    }

    let mut matching_inputs = QueryIter::new(load_cell_lock_hash, Source::Input)
        .enumerate()
        .filter_map(|(index, lock_hash)| (lock_hash == current_lock_hash).then_some(index));

    let Some(input_index) = matching_inputs.next() else {
        return Err(Error::InvalidNftInput);
    };
    if matching_inputs.next().is_some() {
        return Err(Error::InvalidNftInput);
    }

    Ok(input_index)
}

fn verify_offered_nft_input(
    input_index: usize,
    offered_nft_type_hash: [u8; 32],
) -> Result<(), Error> {
    let Some(type_hash) = load_cell_type_hash(input_index, Source::Input)? else {
        return Err(Error::InvalidNftInput);
    };
    if type_hash != offered_nft_type_hash {
        return Err(Error::InvalidNftInput);
    }
    Ok(())
}

pub fn otx_fill_layout(
    origin: &ActionOrigin,
    input_index: usize,
) -> Result<OtxMessageLayout, Error> {
    let ActionOrigin::Otx { layout, .. } = origin else {
        return Err(Error::InvalidCobuild);
    };
    if !range_contains(layout.base_inputs, input_index)? {
        return Err(Error::InvalidCobuild);
    }
    Ok(*layout)
}

fn collect_payments(order: &OrderArgs, layout: OtxMessageLayout) -> Result<Vec<UdtPayment>, Error> {
    let mut payments = Vec::new();
    collect_payments_from_range(order, layout.base_outputs, &mut payments)?;
    collect_payments_from_range(order, layout.append_outputs, &mut payments)?;
    Ok(payments)
}

fn collect_payments_from_range(
    order: &OrderArgs,
    range: Range,
    payments: &mut Vec<UdtPayment>,
) -> Result<(), Error> {
    let end = range
        .start
        .checked_add(range.count)
        .ok_or(Error::InvalidCobuild)?;

    for index in range.start..end {
        let Some(asset_id) = load_cell_type_hash(index, Source::Output)? else {
            continue;
        };
        let owner_lock_hash = load_cell_lock_hash(index, Source::Output)?;
        if !payment_output_matches_order(order, owner_lock_hash, asset_id) {
            continue;
        }

        let data = load_cell_data(index, Source::Output)?;
        if data.len() != UDT_PAYMENT_DATA_LEN {
            continue;
        }
        payments.push(UdtPayment {
            owner_lock_hash,
            asset_id,
            amount: parse_udt_payment(&data)?,
        });
    }

    Ok(())
}

fn payment_output_matches_order(
    order: &OrderArgs,
    owner_lock_hash: [u8; 32],
    asset_id: [u8; 32],
) -> bool {
    owner_lock_hash == order.owner_lock_hash && asset_id == order.requested_asset_id
}

fn range_contains(range: Range, index: usize) -> Result<bool, Error> {
    let end = range
        .start
        .checked_add(range.count)
        .ok_or(Error::InvalidCobuild)?;
    Ok(index >= range.start && index < end)
}

#[allow(dead_code)]
fn _source_marker(_: Source) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> OtxMessageLayout {
        OtxMessageLayout {
            base_inputs: Range { start: 1, count: 1 },
            append_inputs: Range { start: 2, count: 1 },
            base_outputs: Range { start: 0, count: 1 },
            append_outputs: Range { start: 1, count: 1 },
            base_cell_deps: Range { start: 0, count: 0 },
            append_cell_deps: Range { start: 0, count: 0 },
            base_header_deps: Range { start: 0, count: 0 },
            append_header_deps: Range { start: 0, count: 0 },
        }
    }

    #[test]
    fn otx_fill_layout_accepts_current_input_in_base_scope() {
        let origin = ActionOrigin::Otx {
            witness_index: 0,
            otx_index: 0,
            layout: layout(),
        };

        assert_eq!(
            otx_fill_layout(&origin, 1).map(|layout| layout.append_outputs),
            Ok(Range { start: 1, count: 1 })
        );
    }

    #[test]
    fn otx_fill_layout_rejects_tx_level_action() {
        assert_eq!(
            otx_fill_layout(&ActionOrigin::TxLevel { witness_index: 0 }, 1),
            Err(Error::InvalidCobuild)
        );
    }

    #[test]
    fn otx_fill_layout_rejects_append_only_current_input() {
        let origin = ActionOrigin::Otx {
            witness_index: 0,
            otx_index: 0,
            layout: layout(),
        };

        assert_eq!(otx_fill_layout(&origin, 2), Err(Error::InvalidCobuild));
    }

    #[test]
    fn range_contains_accepts_start_and_last_index() {
        assert_eq!(range_contains(Range { start: 3, count: 2 }, 3), Ok(true));
        assert_eq!(range_contains(Range { start: 3, count: 2 }, 4), Ok(true));
        assert_eq!(range_contains(Range { start: 3, count: 2 }, 5), Ok(false));
    }

    #[test]
    fn range_contains_rejects_overflowing_range() {
        assert_eq!(
            range_contains(
                Range {
                    start: usize::MAX,
                    count: 1,
                },
                usize::MAX
            ),
            Err(Error::InvalidCobuild)
        );
    }

    #[test]
    fn payment_output_matches_order_identity_requires_owner_and_asset() {
        let order = crate::types::OrderArgs {
            owner_lock_hash: [2; 32],
            offered_nft_type_hash: [3; 32],
            requested_asset_id: [4; 32],
            min_requested_amount: 30,
        };

        assert!(payment_output_matches_order(&order, [2; 32], [4; 32]));
        assert!(!payment_output_matches_order(&order, [9; 32], [4; 32]));
        assert!(!payment_output_matches_order(&order, [2; 32], [9; 32]));
    }
}

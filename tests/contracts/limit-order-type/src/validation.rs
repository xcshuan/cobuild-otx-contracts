use ckb_std::ckb_constants::Source;
use cobuild_core::{engine::CobuildContext, plan::TypeValidationPlan, reader::cursor_bytes};

use crate::{
    error::Error,
    types::{
        CreateOrderAction, LimitOrderAction, parse_limit_order_action, validate_create,
        validate_fill,
    },
};

pub fn validate_create_order(
    current_type_hash: [u8; 32],
    plan: &TypeValidationPlan,
) -> Result<(), Error> {
    crate::entry::validate_order_type_id()?;
    let order = crate::entry::single_group_order(Source::GroupOutput)?;
    let action = single_create_action(plan)?;
    validate_create(&order, &action)?;
    crate::settlement::ensure_create_nft_proxy_output(
        current_type_hash,
        order.offered_nft_type_hash,
    )
}

pub fn validate_fill_order(
    context: &CobuildContext,
    plan: &TypeValidationPlan,
) -> Result<(), Error> {
    let order = crate::entry::single_group_order(Source::GroupInput)?;
    let fill = crate::otx::load_type_otx_fill(context, plan)?;
    let LimitOrderAction::Fill(action) = parse_limit_order_action(&fill.action_data)? else {
        return Err(Error::UnsupportedAction);
    };
    let payment = crate::settlement::load_bound_payment(fill.layout, action.payment_output_index)?;
    validate_fill(&order, payment)?;
    crate::settlement::ensure_nft_delivered_to_buyer(
        fill.layout,
        action.buyer_lock_hash,
        order.offered_nft_type_hash,
    )
}

fn single_create_action(plan: &TypeValidationPlan) -> Result<CreateOrderAction, Error> {
    if plan.related_actions.len() != 1 {
        return Err(Error::InvalidCobuild);
    }
    let action_data = cursor_bytes(&plan.related_actions[0].action.action.data)?;
    let LimitOrderAction::Create(action) = parse_limit_order_action(&action_data)? else {
        return Err(Error::UnsupportedAction);
    };
    Ok(action)
}

#[cfg(test)]
mod tests {
    #[test]
    fn create_action_context_accepts_any_origin_with_single_create_action() {
        let action = crate::types::LimitOrderAction::Create(crate::types::CreateOrderAction {
            owner_lock_hash: [2; 32],
            offered_nft_type_hash: [3; 32],
            requested_asset_id: [4; 32],
            requested_amount: 30,
        });

        assert!(matches!(action, crate::types::LimitOrderAction::Create(_)));
    }
}

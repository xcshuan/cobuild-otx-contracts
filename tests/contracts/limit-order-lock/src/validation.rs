use cobuild_core::engine::CobuildContext;

use crate::{
    error::Error,
    types::{OrderArgs, parse_fill_order_action, validate_fill},
};

pub fn validate_fill_order(
    context: &CobuildContext,
    order: &OrderArgs,
    current_lock_hash: [u8; 32],
) -> Result<(), Error> {
    let input_index =
        crate::input::load_current_order_input(current_lock_hash, order.offered_nft_type_hash)?;
    let fill = crate::otx::load_lock_otx_fill(context, input_index)?;
    let action = parse_fill_order_action(&fill.action_data)?;
    let payment = crate::settlement::load_bound_payment(fill.layout, action.payment_output_index)?;

    validate_fill(order, payment)?;
    crate::settlement::ensure_nft_delivered_to_buyer(
        fill.layout,
        action.buyer_lock_hash,
        order.offered_nft_type_hash,
    )
}

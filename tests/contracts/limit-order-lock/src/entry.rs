use ckb_std::{
    ckb_types::{bytes::Bytes, prelude::*},
    high_level::{load_script, load_script_hash},
};
use cobuild_core::{
    context::CurrentScript,
    engine::CobuildContext,
};

use crate::{
    error::Error,
    types::{parse_fill_order_action, parse_order_args, validate_fill},
};

pub fn main() -> Result<(), Error> {
    let script = load_script()?;
    let args: Bytes = script.args().unpack();
    let order = parse_order_args(&args)?;

    let current_lock_hash = load_script_hash()?;
    let input_index =
        crate::input::load_current_order_input(current_lock_hash, order.offered_nft_type_hash)?;

    let context = CobuildContext::build(CurrentScript::InputLock(current_lock_hash))?;
    let fill = crate::otx::load_lock_otx_fill(&context, input_index)?;
    let action = parse_fill_order_action(&fill.action_data)?;
    let payment = crate::settlement::load_bound_payment(fill.layout, action.payment_output_index)?;

    validate_fill(&order, payment)?;
    crate::settlement::ensure_nft_delivered_to_buyer(
        fill.layout,
        action.buyer_lock_hash,
        order.offered_nft_type_hash,
    )
}

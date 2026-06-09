use ckb_std::{
    ckb_types::{bytes::Bytes, prelude::*},
    high_level::{load_script, load_script_hash},
};
use cobuild_core::{context::CurrentScript, engine::CobuildContext};

use crate::{error::Error, types::parse_order_args};

pub fn main() -> Result<(), Error> {
    let script = load_script()?;
    let args: Bytes = script.args().unpack();
    let order = parse_order_args(&args)?;

    let current_lock_hash = load_script_hash()?;
    let context = CobuildContext::build(CurrentScript::InputLock(current_lock_hash))?;
    crate::validation::validate_fill_order(&context, &order, current_lock_hash)
}

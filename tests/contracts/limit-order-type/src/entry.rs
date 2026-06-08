use alloc::vec::Vec;

use ckb_std::{
    ckb_constants::Source,
    high_level::{QueryIter, load_cell_data, load_cell_lock_hash, load_script_hash},
};
use cobuild_core::{
    context::CurrentScript,
    engine::CobuildContext,
    layout::Range,
    plan::{ActionOrigin, OtxMessageLayout, OtxTypeRelation},
    reader::cursor_bytes,
};

use crate::{
    error::Error,
    types::{
        SETTLEMENT_DATA_LEN, SettlementCell, parse_fill_order_action, parse_order_state,
        parse_settlement_cell, validate_fill,
    },
};

pub fn main() -> Result<(), Error> {
    let current_type_hash = load_script_hash()?;
    let plan =
        CobuildContext::build(CurrentScript::Type(current_type_hash))?.plan_type_validation()?;

    let order = single_input_order()?;
    require_no_order_output()?;

    if plan.related_actions.len() != 1 {
        return Err(Error::InvalidCobuild);
    }
    let related = &plan.related_actions[0];
    let layout = otx_fill_layout(&related.action.origin, related.otx_relation)?;
    let action_data = cursor_bytes(&related.action.action.data)?;
    let action = parse_fill_order_action(&action_data)?;
    let settlements = collect_settlements(layout)?;

    validate_fill(&order, &action, &settlements)
}

fn single_input_order() -> Result<crate::types::OrderState, Error> {
    let mut inputs = QueryIter::new(load_cell_data, Source::GroupInput);
    let Some(data) = inputs.next() else {
        return Err(Error::InvalidOrderData);
    };
    if inputs.next().is_some() {
        return Err(Error::InvalidOrderData);
    }

    parse_order_state(&data)
}

fn require_no_order_output() -> Result<(), Error> {
    if QueryIter::new(load_cell_data, Source::GroupOutput)
        .next()
        .is_some()
    {
        return Err(Error::InvalidOrderData);
    }

    Ok(())
}

pub fn otx_fill_layout(
    origin: &ActionOrigin,
    relation: Option<OtxTypeRelation>,
) -> Result<OtxMessageLayout, Error> {
    let ActionOrigin::Otx { layout, .. } = origin else {
        return Err(Error::InvalidCobuild);
    };
    let Some(relation) = relation else {
        return Err(Error::InvalidCobuild);
    };
    if !relation.input_type_in_base {
        return Err(Error::InvalidCobuild);
    }

    Ok(*layout)
}

fn collect_settlements(layout: OtxMessageLayout) -> Result<Vec<SettlementCell>, Error> {
    let mut settlements = Vec::new();
    collect_settlements_from_range(layout.base_outputs, &mut settlements)?;
    collect_settlements_from_range(layout.append_outputs, &mut settlements)?;
    Ok(settlements)
}

fn collect_settlements_from_range(
    range: Range,
    settlements: &mut Vec<SettlementCell>,
) -> Result<(), Error> {
    let end = range
        .start
        .checked_add(range.count)
        .ok_or(Error::InvalidCobuild)?;

    for index in range.start..end {
        let data = load_cell_data(index, Source::Output)?;
        if data.len() != SETTLEMENT_DATA_LEN {
            continue;
        }

        let lock_hash = load_cell_lock_hash(index, Source::Output)?;
        settlements.push(parse_settlement_cell(lock_hash, &data)?);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobuild_core::{
        layout::Range,
        plan::{ActionOrigin, OtxMessageLayout, OtxTypeRelation},
    };

    fn layout() -> OtxMessageLayout {
        OtxMessageLayout {
            base_inputs: Range { start: 0, count: 1 },
            append_inputs: Range { start: 1, count: 0 },
            base_outputs: Range { start: 0, count: 0 },
            append_outputs: Range { start: 0, count: 1 },
            base_cell_deps: Range { start: 0, count: 0 },
            append_cell_deps: Range { start: 0, count: 0 },
            base_header_deps: Range { start: 0, count: 0 },
            append_header_deps: Range { start: 0, count: 0 },
        }
    }

    fn relation(input_type_in_base: bool) -> OtxTypeRelation {
        OtxTypeRelation {
            input_type_in_base,
            input_type_in_append: false,
            output_type_in_base: false,
            output_type_in_base_covered: false,
            output_type_in_append: false,
        }
    }

    #[test]
    fn otx_fill_context_accepts_base_input_relation() {
        let origin = ActionOrigin::Otx {
            witness_index: 0,
            otx_index: 0,
            layout: layout(),
        };

        assert_eq!(
            otx_fill_layout(&origin, Some(relation(true))).map(|layout| layout.append_outputs),
            Ok(Range { start: 0, count: 1 })
        );
    }

    #[test]
    fn otx_fill_context_rejects_tx_level_action() {
        let origin = ActionOrigin::TxLevel { witness_index: 0 };

        assert_eq!(
            otx_fill_layout(&origin, None),
            Err(crate::error::Error::InvalidCobuild)
        );
    }

    #[test]
    fn otx_fill_context_rejects_non_base_input_relation() {
        let origin = ActionOrigin::Otx {
            witness_index: 0,
            otx_index: 0,
            layout: layout(),
        };

        assert_eq!(
            otx_fill_layout(&origin, Some(relation(false))),
            Err(crate::error::Error::InvalidCobuild)
        );
    }
}

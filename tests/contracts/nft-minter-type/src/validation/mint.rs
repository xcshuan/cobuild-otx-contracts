use alloc::vec::Vec;

use ckb_std::{
    ckb_constants::Source,
    ckb_types::prelude::*,
    high_level::{QueryIter, load_cell_data, load_cell_lock_hash, load_cell_type},
};
use cobuild_core::{
    plan::{ActionOrigin, ActionRef, TypeValidationPlan},
    protocol::ScriptRole,
    reader::cursor_bytes,
};

use crate::{
    error::Error,
    types::{MinterState, NftMinterAction, parse_action},
    validation::helpers::single_group_state,
};

pub fn validate_mint(current_type_hash: [u8; 32], plan: &TypeValidationPlan) -> Result<(), Error> {
    let input = single_group_state(Source::GroupInput)?;
    let output = single_group_state(Source::GroupOutput)?;
    let actions = mint_actions(plan)?;
    validate_mint_state(input, output, actions.len())?;
    validate_expected_outputs(current_type_hash, input.mint_counter, &actions)
}

pub fn validate_mint_state(
    input: MinterState,
    output: MinterState,
    mint_action_count: usize,
) -> Result<(), Error> {
    if input.supply_cap != output.supply_cap {
        return Err(Error::SupplyCap);
    }
    let increment = mint_action_count as u64;
    let expected = input
        .mint_counter
        .checked_add(increment)
        .ok_or(Error::Counter)?;
    if output.mint_counter != expected {
        return Err(Error::Counter);
    }
    if output.mint_counter > output.supply_cap {
        return Err(Error::SupplyCap);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MintActionFact {
    pub action_ref: ActionRef,
    pub metadata_seed: [u8; 32],
    pub mint_to_lock_hash: [u8; 32],
    pub output_candidates: OutputCandidates,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputCandidates {
    All,
    OtxRanges {
        base_start: usize,
        base_end: usize,
        append_start: usize,
        append_end: usize,
    },
}

pub fn mint_actions(plan: &TypeValidationPlan) -> Result<Vec<MintActionFact>, Error> {
    let mut facts = Vec::new();
    for related in &plan.related_actions {
        if related.action.action.script_role != ScriptRole::InputType {
            return Err(Error::InvalidAction);
        }
        let action_data = cursor_bytes(&related.action.action.data)?;
        let NftMinterAction::MintNft {
            metadata_seed,
            mint_to_lock_hash,
        } = parse_action(&action_data)?
        else {
            return Err(Error::InvalidAction);
        };
        let output_candidates = match related.action.origin {
            ActionOrigin::TxLevel { .. } => OutputCandidates::All,
            ActionOrigin::Otx { layout, .. } => OutputCandidates::OtxRanges {
                base_start: layout.base_outputs.start,
                base_end: layout.base_outputs.end(),
                append_start: layout.append_outputs.start,
                append_end: layout.append_outputs.end(),
            },
        };
        facts.push(MintActionFact {
            action_ref: related.action.action_ref(),
            metadata_seed,
            mint_to_lock_hash,
            output_candidates,
        });
    }
    facts.sort_by_key(|fact| fact.action_ref);
    Ok(facts)
}

fn validate_expected_outputs(
    current_type_hash: [u8; 32],
    old_counter: u64,
    actions: &[MintActionFact],
) -> Result<(), Error> {
    for (offset, action) in actions.iter().enumerate() {
        let offset = offset as u64;
        let serial = old_counter.checked_add(offset).ok_or(Error::Counter)?;
        let rarity = crate::types::rarity_for_serial(serial);
        let expected_id = crate::types::nft_id(current_type_hash, serial);
        let expected_attributes =
            crate::types::attributes_hash(current_type_hash, serial, rarity, action.metadata_seed);
        let mut matches = 0usize;

        match action.output_candidates {
            OutputCandidates::All => {
                for (index, type_script) in
                    QueryIter::new(load_cell_type, Source::Output).enumerate()
                {
                    matches += validate_expected_output_at(
                        index,
                        type_script,
                        current_type_hash,
                        &expected_id,
                        serial,
                        rarity,
                        expected_attributes,
                        action.mint_to_lock_hash,
                    )?;
                }
            }
            OutputCandidates::OtxRanges {
                base_start,
                base_end,
                append_start,
                append_end,
            } => {
                matches += validate_expected_outputs_in_range(
                    base_start,
                    base_end,
                    current_type_hash,
                    &expected_id,
                    serial,
                    rarity,
                    expected_attributes,
                    action.mint_to_lock_hash,
                )?;
                matches += validate_expected_outputs_in_range(
                    append_start,
                    append_end,
                    current_type_hash,
                    &expected_id,
                    serial,
                    rarity,
                    expected_attributes,
                    action.mint_to_lock_hash,
                )?;
            }
        }

        if matches != 1 {
            return Err(Error::InvalidMintedNft);
        }
    }
    Ok(())
}

fn validate_expected_outputs_in_range(
    start: usize,
    end: usize,
    current_type_hash: [u8; 32],
    expected_id: &[u8; 32],
    serial: u64,
    rarity: u8,
    expected_attributes: [u8; 32],
    expected_lock_hash: [u8; 32],
) -> Result<usize, Error> {
    let mut matches = 0usize;
    for index in start..end {
        let type_script = load_cell_type(index, Source::Output)?;
        matches += validate_expected_output_at(
            index,
            type_script,
            current_type_hash,
            expected_id,
            serial,
            rarity,
            expected_attributes,
            expected_lock_hash,
        )?;
    }
    Ok(matches)
}

fn validate_expected_output_at(
    index: usize,
    type_script: Option<ckb_std::ckb_types::packed::Script>,
    current_type_hash: [u8; 32],
    expected_id: &[u8; 32],
    serial: u64,
    rarity: u8,
    expected_attributes: [u8; 32],
    expected_lock_hash: [u8; 32],
) -> Result<usize, Error> {
    let Some(type_script) = type_script else {
        return Ok(0);
    };
    let args: Vec<u8> = type_script.args().unpack();
    if args.as_slice() != expected_id.as_slice() {
        return Ok(0);
    }

    let data = load_cell_data(index, Source::Output)?;
    let nft = crate::types::parse_minted_nft_data(&data)?;
    if nft.minter_type_hash != current_type_hash
        || nft.serial != serial
        || nft.rarity != rarity
        || nft.attributes_hash != expected_attributes
    {
        return Err(Error::InvalidMintedNft);
    }
    let lock_hash = load_cell_lock_hash(index, Source::Output)?;
    if lock_hash != expected_lock_hash {
        return Err(Error::InvalidMintedNft);
    }
    Ok(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use cobuild_core::{
        layout::Range,
        plan::{
            ActionOrigin, ActionRef, OtxMessageLayout, RelatedAction, TypeActionOtxScope,
            TypeRelatedAction, TypeValidationPlan,
        },
        protocol::ScriptRole,
        reader::cursor_from_slice,
        view::ActionView,
    };

    use crate::types::{create_minter_action_data, mint_nft_action_data};

    fn layout() -> OtxMessageLayout {
        OtxMessageLayout {
            base_inputs: Range { start: 0, count: 1 },
            append_inputs: Range { start: 1, count: 0 },
            base_outputs: Range { start: 0, count: 1 },
            append_outputs: Range { start: 1, count: 0 },
            base_cell_deps: Range { start: 0, count: 0 },
            append_cell_deps: Range { start: 0, count: 0 },
            base_header_deps: Range { start: 0, count: 0 },
            append_header_deps: Range { start: 0, count: 0 },
        }
    }

    fn related_action(
        origin: ActionOrigin,
        index: usize,
        script_role: ScriptRole,
        data: &[u8],
    ) -> TypeRelatedAction {
        TypeRelatedAction {
            action: RelatedAction {
                origin,
                action: ActionView {
                    index,
                    script_info_hash: [0; 32],
                    script_role,
                    script_hash: [1; 32],
                    data: cursor_from_slice(data),
                },
            },
            otx_type_scope: TypeActionOtxScope::TargetOnly,
        }
    }

    fn plan(related_actions: Vec<TypeRelatedAction>) -> TypeValidationPlan {
        TypeValidationPlan {
            type_script_hash: [1; 32],
            related_actions,
        }
    }

    #[test]
    fn mint_transition_requires_counter_increment_and_fixed_cap() {
        let input = MinterState {
            mint_counter: 6,
            supply_cap: 10,
        };
        let output = MinterState {
            mint_counter: 8,
            supply_cap: 10,
        };
        assert_eq!(validate_mint_state(input, output, 2), Ok(()));
    }

    #[test]
    fn mint_transition_rejects_wrong_counter_cap_and_over_cap() {
        assert_eq!(
            validate_mint_state(
                MinterState {
                    mint_counter: 6,
                    supply_cap: 10,
                },
                MinterState {
                    mint_counter: 7,
                    supply_cap: 10,
                },
                2,
            ),
            Err(Error::Counter)
        );
        assert_eq!(
            validate_mint_state(
                MinterState {
                    mint_counter: 6,
                    supply_cap: 10,
                },
                MinterState {
                    mint_counter: 8,
                    supply_cap: 11,
                },
                2,
            ),
            Err(Error::SupplyCap)
        );
        assert_eq!(
            validate_mint_state(
                MinterState {
                    mint_counter: 9,
                    supply_cap: 10,
                },
                MinterState {
                    mint_counter: 11,
                    supply_cap: 10,
                },
                2,
            ),
            Err(Error::SupplyCap)
        );
    }

    #[test]
    fn mint_actions_extracts_and_sorts_mint_facts() {
        let seed_a = [1; 32];
        let seed_b = [2; 32];
        let seed_c = [3; 32];
        let mint_to_a = [11; 32];
        let mint_to_b = [12; 32];
        let mint_to_c = [13; 32];
        let action_a = mint_nft_action_data(seed_a, mint_to_a);
        let action_b = mint_nft_action_data(seed_b, mint_to_b);
        let action_c = mint_nft_action_data(seed_c, mint_to_c);
        let plan = plan(vec![
            related_action(
                ActionOrigin::TxLevel { witness_index: 5 },
                2,
                ScriptRole::InputType,
                &action_b,
            ),
            related_action(
                ActionOrigin::Otx {
                    witness_index: 3,
                    otx_index: 0,
                    layout: layout(),
                },
                1,
                ScriptRole::InputType,
                &action_a,
            ),
            related_action(
                ActionOrigin::TxLevel { witness_index: 3 },
                0,
                ScriptRole::InputType,
                &action_c,
            ),
        ]);

        assert_eq!(
            mint_actions(&plan),
            Ok(vec![
                MintActionFact {
                    action_ref: ActionRef::TxLevel {
                        witness_index: 3,
                        action_index: 0,
                    },
                    metadata_seed: seed_c,
                    mint_to_lock_hash: mint_to_c,
                    output_candidates: OutputCandidates::All,
                },
                MintActionFact {
                    action_ref: ActionRef::Otx {
                        witness_index: 3,
                        otx_index: 0,
                        action_index: 1,
                    },
                    metadata_seed: seed_a,
                    mint_to_lock_hash: mint_to_a,
                    output_candidates: OutputCandidates::OtxRanges {
                        base_start: 0,
                        base_end: 1,
                        append_start: 1,
                        append_end: 1,
                    },
                },
                MintActionFact {
                    action_ref: ActionRef::TxLevel {
                        witness_index: 5,
                        action_index: 2,
                    },
                    metadata_seed: seed_b,
                    mint_to_lock_hash: mint_to_b,
                    output_candidates: OutputCandidates::All,
                },
            ])
        );
    }

    #[test]
    fn mint_actions_rejects_non_mint_actions() {
        let action = create_minter_action_data(10);
        let plan = plan(vec![related_action(
            ActionOrigin::TxLevel { witness_index: 0 },
            0,
            ScriptRole::InputType,
            &action,
        )]);

        assert_eq!(mint_actions(&plan), Err(Error::InvalidAction));
    }

    #[test]
    fn mint_actions_rejects_output_type_mint_action() {
        let action = mint_nft_action_data([4; 32], [5; 32]);
        let plan = plan(vec![related_action(
            ActionOrigin::TxLevel { witness_index: 0 },
            0,
            ScriptRole::OutputType,
            &action,
        )]);

        assert_eq!(mint_actions(&plan), Err(Error::InvalidAction));
    }
}

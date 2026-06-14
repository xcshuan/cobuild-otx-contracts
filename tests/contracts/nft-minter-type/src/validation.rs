use alloc::vec::Vec;

use ckb_std::{
    ckb_constants::Source,
    high_level::{load_cell_data, QueryIter},
};
use cobuild_core::{
    plan::{ActionOrigin, TypeValidationPlan},
    protocol::ScriptRole,
    reader::cursor_bytes,
};

use crate::{
    error::Error,
    types::{parse_action, parse_minter_state, MinterState, NftMinterAction},
};

pub fn validate_create(plan: &TypeValidationPlan) -> Result<(), Error> {
    crate::entry::validate_minter_type_id()?;
    let output = single_group_state(Source::GroupOutput)?;
    let action = single_action(plan)?;
    validate_create_state(output, action)
}

pub fn validate_create_state(output: MinterState, action: NftMinterAction) -> Result<(), Error> {
    let NftMinterAction::CreateMinter { supply_cap } = action else {
        return Err(Error::InvalidAction);
    };
    if output.mint_counter != 0 {
        return Err(Error::Counter);
    }
    if output.supply_cap != supply_cap {
        return Err(Error::SupplyCap);
    }
    Ok(())
}

pub fn validate_mint(current_type_hash: [u8; 32], plan: &TypeValidationPlan) -> Result<(), Error> {
    let input = single_group_state(Source::GroupInput)?;
    let output = single_group_state(Source::GroupOutput)?;
    let actions = mint_actions(plan)?;
    validate_mint_state(input, output, actions.len())?;
    let _ = current_type_hash;
    Ok(())
}

pub fn validate_mint_state(
    input: MinterState,
    output: MinterState,
    mint_action_count: usize,
) -> Result<(), Error> {
    if input.supply_cap != output.supply_cap {
        return Err(Error::SupplyCap);
    }
    let increment: u64 = mint_action_count.try_into().map_err(|_| Error::Counter)?;
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
    pub witness_index: usize,
    pub action_index: usize,
    pub metadata_seed: [u8; 32],
}

pub fn mint_actions(plan: &TypeValidationPlan) -> Result<Vec<MintActionFact>, Error> {
    let mut facts = Vec::new();
    for related in &plan.related_actions {
        if related.action.action.script_role != ScriptRole::InputType {
            return Err(Error::InvalidAction);
        }
        let action_data = cursor_bytes(&related.action.action.data)?;
        let NftMinterAction::MintNft { metadata_seed } = parse_action(&action_data)? else {
            return Err(Error::InvalidAction);
        };
        let witness_index = match related.action.origin {
            ActionOrigin::TxLevel { witness_index } => witness_index,
            ActionOrigin::Otx { witness_index, .. } => witness_index,
        };
        facts.push(MintActionFact {
            witness_index,
            action_index: related.action.action.index,
            metadata_seed,
        });
    }
    facts.sort_by_key(|fact| (fact.witness_index, fact.action_index));
    Ok(facts)
}

pub fn single_action(plan: &TypeValidationPlan) -> Result<NftMinterAction, Error> {
    if plan.related_actions.len() != 1 {
        return Err(Error::InvalidCobuild);
    }
    let action_data = cursor_bytes(&plan.related_actions[0].action.action.data)?;
    parse_action(&action_data)
}

pub fn single_group_state(source: Source) -> Result<MinterState, Error> {
    let mut cells = QueryIter::new(load_cell_data, source);
    let Some(data) = cells.next() else {
        return Err(Error::InvalidMinterData);
    };
    if cells.next().is_some() {
        return Err(Error::InvalidShape);
    }
    parse_minter_state(&data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use cobuild_core::{
        layout::Range,
        plan::{
            ActionOrigin, OtxMessageLayout, RelatedAction, TypeActionOtxScope, TypeRelatedAction,
            TypeValidationPlan,
        },
        protocol::ScriptRole,
        reader::cursor_from_slice,
        view::ActionView,
    };

    use crate::types::{
        create_minter_action_data, mint_nft_action_data, MinterState, NftMinterAction,
    };

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
    fn create_requires_zero_counter_and_matching_cap() {
        let state = MinterState {
            mint_counter: 0,
            supply_cap: 10,
        };
        let action = NftMinterAction::CreateMinter { supply_cap: 10 };

        assert_eq!(validate_create_state(state, action), Ok(()));
    }

    #[test]
    fn create_rejects_non_zero_counter_or_cap_mismatch() {
        assert_eq!(
            validate_create_state(
                MinterState {
                    mint_counter: 1,
                    supply_cap: 10,
                },
                NftMinterAction::CreateMinter { supply_cap: 10 },
            ),
            Err(Error::Counter)
        );
        assert_eq!(
            validate_create_state(
                MinterState {
                    mint_counter: 0,
                    supply_cap: 9,
                },
                NftMinterAction::CreateMinter { supply_cap: 10 },
            ),
            Err(Error::SupplyCap)
        );
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
        let action_a = mint_nft_action_data(seed_a);
        let action_b = mint_nft_action_data(seed_b);
        let action_c = mint_nft_action_data(seed_c);
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
                    witness_index: 3,
                    action_index: 0,
                    metadata_seed: seed_c,
                },
                MintActionFact {
                    witness_index: 3,
                    action_index: 1,
                    metadata_seed: seed_a,
                },
                MintActionFact {
                    witness_index: 5,
                    action_index: 2,
                    metadata_seed: seed_b,
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
        let action = mint_nft_action_data([4; 32]);
        let plan = plan(vec![related_action(
            ActionOrigin::TxLevel { witness_index: 0 },
            0,
            ScriptRole::OutputType,
            &action,
        )]);

        assert_eq!(mint_actions(&plan), Err(Error::InvalidAction));
    }
}

pub mod assertions;
pub mod cells;
pub mod cobuild;
pub mod contracts;
pub mod fixture;
pub mod scripts;
pub mod signing;
pub mod tx;

#[cfg(test)]
mod tests {
    use super::{
        assertions::{assert_lock_script_exit_result, assert_type_script_exit_result},
        cells::{TestResolvedInput, live_resolved_typed_input},
        cobuild::{CobuildMessageBuilder, OtxBuilder, empty_message, seal_pair},
        contracts::{deploy_always_success, deploy_data2_script},
        fixture::CobuildTestFixture,
        scripts::script_hash,
        signing::{
            fixed_secret_key, public_key_hash20, sighash_all_only_witness, sign_recoverable,
        },
        tx::otx_start_witness,
    };
    use crate::fixtures::limit_order::{
        LimitOrderCobuildMessageExt, LimitOrderFixtureExt, LimitOrderState, order_data,
        settlement_data,
    };
    use ckb_testtool::{
        ckb_script::ScriptError,
        ckb_types::{
            packed::{CellInput, OutPoint},
            prelude::{Builder, Entity, Pack, Unpack},
        },
        context::Context,
    };

    #[test]
    fn limit_order_helpers_encode_fixed_width_order_and_settlement_data() {
        let order = LimitOrderState {
            order_id: [1; 32],
            owner_lock_hash: [2; 32],
            offered_asset_id: [3; 32],
            requested_asset_id: [4; 32],
            offered_remaining: 10,
            min_requested_per_offered: 3,
            nonce: 9,
        };

        let data = order_data(order);
        let settlement = settlement_data([4; 32], 30);

        assert_eq!(data.len(), 152);
        assert_eq!(&data[0..32], &[1; 32]);
        assert_eq!(&data[32..64], &[2; 32]);
        assert_eq!(&data[64..96], &[3; 32]);
        assert_eq!(&data[96..128], &[4; 32]);
        assert_eq!(&data[128..136], &10u64.to_le_bytes());
        assert_eq!(&data[136..144], &3u64.to_le_bytes());
        assert_eq!(&data[144..152], &9u64.to_le_bytes());
        assert_eq!(settlement.len(), 40);
        assert_eq!(&settlement[0..32], &[4; 32]);
        assert_eq!(&settlement[32..40], &30u64.to_le_bytes());
    }

    #[test]
    fn limit_order_fixture_encodes_fill_action_and_default_otx_layout() {
        let message = CobuildMessageBuilder::new()
            .input_type_action([9; 32])
            .limit_order_fill([1; 32], [4; 32], 10, 30)
            .build();

        let fixture = CobuildTestFixture::new();
        let otx = fixture
            .limit_order_append_settlement_otx()
            .message(message)
            .build();

        assert_eq!(otx.append_permissions().as_slice(), &[0b0010]);
    }

    #[test]
    fn otx_witness_helpers_encode_start_and_seal() {
        let message = empty_message();
        assert_eq!(message.actions().len(), 0);

        let seal = seal_pair([9u8; 32], 0, vec![1, 2, 3]);
        assert_eq!(seal.script_hash().raw_data().as_ref(), &[9u8; 32]);

        let witness = otx_start_witness(1, 2, 3, 4);
        assert!(!witness.is_empty());
    }

    #[test]
    fn signing_helpers_build_sighash_all_only_witness() {
        let secret_key = fixed_secret_key(1);
        let public_key_hash = public_key_hash20(&secret_key);
        assert_eq!(public_key_hash.len(), 20);

        let seal = sign_recoverable(&secret_key, [7u8; 32]);
        assert_eq!(seal.len(), 65);

        let witness = sighash_all_only_witness(seal.clone());
        assert!(witness.len() > seal.len());
        assert!(
            witness
                .windows(seal.len())
                .any(|window| window == seal.as_slice())
        );
    }

    #[test]
    fn contract_helpers_deploy_scripts_and_record_script_hashes() {
        let mut context = Context::default();

        let limit_order = deploy_data2_script(&mut context, "limit-order-type", Vec::new());
        let always_success = deploy_always_success(&mut context, Vec::new());

        assert_eq!(limit_order.script_hash, script_hash(&limit_order.script));
        assert_eq!(
            always_success.script_hash,
            script_hash(&always_success.script)
        );
        let cell_dep_index: u32 = limit_order.cell_dep.out_point().index().unpack();
        assert_eq!(cell_dep_index, 0);
    }

    #[test]
    fn resolved_input_helpers_preserve_cell_and_data() {
        let mut fixture = CobuildTestFixture::new();
        let lock = fixture.deploy_always_success();
        let type_script = fixture.deploy_always_success();
        let (_input, resolved): (_, TestResolvedInput) = live_resolved_typed_input(
            fixture.context_mut(),
            lock.script.clone(),
            type_script.script.clone(),
            1_000,
            vec![1, 2, 3],
        );

        assert!(!resolved.raw_input.is_empty());
        assert!(!resolved.resolved_output.is_empty());
        assert_eq!(resolved.data, vec![1, 2, 3]);

        let deployed =
            deploy_data2_script(fixture.context_mut(), "cobuild-otx-lock", vec![0u8; 21]);
        assert_eq!(deployed.script.args().raw_data().len(), 21);
    }

    #[test]
    fn type_script_exit_assertion_matches_index_and_exit_code() {
        let error = ScriptError::ValidationFailure("by convention".to_owned(), 11)
            .input_type_script(0)
            .into();

        assert_type_script_exit_result(Err(error), 0, 11);
    }

    #[test]
    fn lock_script_exit_assertion_matches_index_and_exit_code() {
        let error = ScriptError::ValidationFailure("by convention".to_owned(), 39)
            .input_lock_script(0)
            .into();

        assert_lock_script_exit_result(Err(error), 0, 39);
    }

    #[test]
    #[should_panic(expected = "originating script")]
    fn type_script_exit_assertion_rejects_wrong_index() {
        let error = ScriptError::ValidationFailure("by convention".to_owned(), 11)
            .input_type_script(0)
            .into();

        assert_type_script_exit_result(Err(error), 1, 11);
    }

    #[test]
    #[should_panic(expected = "exit code")]
    fn type_script_exit_assertion_rejects_wrong_exit_code() {
        let error = ScriptError::ValidationFailure("by convention".to_owned(), 11)
            .input_type_script(0)
            .into();

        assert_type_script_exit_result(Err(error), 0, 10);
    }

    #[test]
    fn fixture_facade_deploys_contracts_and_starts_builders() {
        let mut fixture = CobuildTestFixture::new();

        let limit_order = fixture.deploy_limit_order();
        let always_success = fixture.deploy_always_success();
        let owner_lock = always_success.script.clone();
        let _order = fixture.limit_order().owner(owner_lock);
        let _message = fixture.cobuild().input_type_action(limit_order.script_hash);
        let _otx = fixture.otx();
        let _tx = fixture.tx();
    }

    #[test]
    #[should_panic(expected = "each OTX requires non-zero base inputs")]
    fn tx_builder_rejects_zero_base_inputs_in_any_otx() {
        let otx = OtxBuilder::new().base_input_cells(0).build_with_layout();
        let input = CellInput::new_builder()
            .previous_output(OutPoint::new([1u8; 32].pack(), 0))
            .build();

        CobuildTestFixture::new()
            .tx()
            .base_input(input)
            .otx(otx)
            .build();
    }
}

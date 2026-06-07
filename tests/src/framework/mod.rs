pub mod assertions;
pub mod cells;
pub mod cobuild;
pub mod contracts;
pub mod fixture;
pub mod scripts;
pub mod tx;

#[cfg(test)]
mod tests {
    use super::{
        assertions::assert_type_script_exit_result,
        cells::{LimitOrderState, order_data, settlement_data},
        cobuild::{CobuildMessageBuilder, OtxBuilder},
        contracts::{deploy_always_success, deploy_data2_script},
        fixture::CobuildTestFixture,
        scripts::script_hash,
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
    fn cobuild_helpers_encode_limit_order_fill_action_and_default_otx_layout() {
        let message = CobuildMessageBuilder::new()
            .input_type_action([9; 32])
            .limit_order_fill([1; 32], [4; 32], 10, 30)
            .build();

        let otx = OtxBuilder::new()
            .message(message)
            .base_input_cells(1)
            .append_output_cells(1)
            .allow_append_outputs()
            .build();

        assert_eq!(otx.append_permissions().as_slice(), &[0b0010]);
    }

    #[test]
    fn contract_helpers_deploy_scripts_and_record_script_hashes() {
        let mut context = Context::default();

        let limit_order = deploy_data2_script(&mut context, "limit-order", Vec::new());
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
    fn type_script_exit_assertion_matches_index_and_exit_code() {
        let error = ScriptError::ValidationFailure("by convention".to_owned(), 11)
            .input_type_script(0)
            .into();

        assert_type_script_exit_result(Err(error), 0, 11);
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

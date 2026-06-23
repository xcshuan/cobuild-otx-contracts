pub mod assertions;
pub mod cells;
pub mod cobuild;
pub mod contracts;
pub mod fixture;
pub mod scenario;
pub mod scripts;
pub mod signing;
pub mod tx;

#[cfg(test)]
mod tests {
    use super::{
        assertions::{assert_lock_script_exit_result, assert_type_script_exit_result},
        cells::{
            TestCellOutput, TestResolvedInput, live_resolved_facts, live_resolved_typed_input,
            normal_output, typed_output,
        },
        cobuild::{CobuildMessageBuilder, OtxBuilder, empty_message, lock_seal},
        contracts::{
            DeployedScript, build_deployed_script, deploy_loader_binary_code,
            deploy_script_bytes_code,
        },
        fixture::CobuildTestFixture,
        scripts::script_hash,
        signing::{
            fixed_secret_key, public_key_hash20, sighash_all_only_witness, sign_recoverable,
        },
        tx::{OtxSpec, TxShape, append_segment_spec, otx_start_witness},
    };
    use ckb_testtool::{
        ckb_script::ScriptError,
        ckb_types::{bytes::Bytes, core::ScriptHashType, prelude::Unpack},
        context::Context,
    };

    fn deploy_protocol_dummy_script(
        context: &mut Context,
        tag: u8,
        args: Vec<u8>,
    ) -> DeployedScript {
        let code = deploy_script_bytes_code(context, Bytes::from(vec![tag]), ScriptHashType::Data);
        build_deployed_script(context, &code, ScriptHashType::Data, args)
    }

    #[test]
    fn otx_witness_helpers_encode_start_and_seal() {
        let message = empty_message();
        assert_eq!(message.actions().len(), 0);

        let seal = lock_seal([9u8; 32], vec![1, 2, 3]);
        assert_eq!(seal.script_hash().raw_data().as_ref(), &[9u8; 32]);

        let witness = otx_start_witness(1, 2, 3, 4);
        assert!(!witness.is_empty());
    }

    #[test]
    fn otx_builder_allows_append_inputs_and_outputs() {
        let otx = OtxBuilder::new()
            .base_input_cells(2)
            .base_output_cells(1)
            .append_segment(0, 1, 2, 0, 0, Vec::new())
            .allow_append_inputs()
            .allow_append_outputs()
            .build_with_layout();

        assert_eq!(otx.base_input_cells, 2);
        assert_eq!(otx.base_output_cells, 1);
        assert_eq!(otx.append_input_cells, 1);
        assert_eq!(otx.append_output_cells, 2);
        assert_eq!(otx.append_segments[0].input_cells, 1);
        assert_eq!(otx.append_segments[0].output_cells, 2);
        assert_eq!(otx.otx.append_permissions().as_slice(), &[0b0011]);
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

        let data2_code =
            deploy_script_bytes_code(&mut context, Bytes::from(vec![0x42]), ScriptHashType::Data2);
        let data2_script =
            build_deployed_script(&mut context, &data2_code, ScriptHashType::Data2, Vec::new());
        let dummy_script = deploy_protocol_dummy_script(&mut context, 3, Vec::new());

        assert_eq!(data2_script.script_hash, script_hash(&data2_script.script));
        assert_eq!(dummy_script.script_hash, script_hash(&dummy_script.script));
        let cell_dep_index: u32 = data2_script.cell_dep.out_point().index().unpack();
        assert_eq!(cell_dep_index, 0);
    }

    #[test]
    fn resolved_input_helpers_preserve_cell_and_data() {
        let mut fixture = CobuildTestFixture::new();
        let lock = deploy_protocol_dummy_script(fixture.context_mut(), 4, Vec::new());
        let type_script = deploy_protocol_dummy_script(fixture.context_mut(), 5, Vec::new());
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

        let facts = live_resolved_facts(
            fixture.context_mut(),
            typed_output(lock.script.clone(), type_script.script.clone(), 2_000),
            vec![4, 5, 6],
        );
        assert_eq!(facts.data, vec![4, 5, 6]);
        assert_eq!(facts.lock_hash, script_hash(&lock.script));
        assert_eq!(facts.type_hash, Some(script_hash(&type_script.script)));

        let code = deploy_loader_binary_code(
            fixture.context_mut(),
            "cobuild-otx-lock",
            ScriptHashType::Data2,
        );
        let deployed = build_deployed_script(
            fixture.context_mut(),
            &code,
            ScriptHashType::Data2,
            vec![0u8; 21],
        );
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
    fn output_type_script_exit_assertion_matches_index_and_exit_code() {
        let error = ScriptError::ValidationFailure("by convention".to_owned(), 14)
            .output_type_script(0)
            .into();

        super::assertions::assert_output_type_script_exit_result(Err(error), 0, 14);
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

        let script = deploy_protocol_dummy_script(fixture.context_mut(), 6, Vec::new());
        let _message = fixture
            .cobuild()
            .input_lock_action(script_hash(&script.script));
        let _otx = fixture.otx();
    }

    #[test]
    #[should_panic(expected = "OTX segment requires non-zero base inputs")]
    fn tx_shape_rejects_zero_base_inputs_in_any_otx() {
        let mut shape = TxShape::new();
        shape.push_otx(OtxSpec::default());
    }

    #[test]
    fn tx_shape_supports_sighash_all_message_without_otx() {
        let mut fixture = CobuildTestFixture::new();
        let lock = deploy_protocol_dummy_script(fixture.context_mut(), 7, Vec::new());
        let input = live_resolved_facts(
            fixture.context_mut(),
            normal_output(lock.script.clone(), 1_000),
            Vec::new(),
        );
        let output = TestCellOutput::new(normal_output(lock.script, 900), Vec::new());
        let message = CobuildMessageBuilder::new()
            .output_type_action([9; 32])
            .action_data(vec![1])
            .build();

        let mut shape = TxShape::new();
        shape.push_prefix_input(input);
        shape.push_remainder_output(output);
        shape.tx_level_message(message);
        let built = shape.build();

        assert_eq!(built.tx.inputs().len(), 1);
        assert_eq!(built.tx.outputs().len(), 1);
        assert_eq!(built.tx.witnesses().len(), 1);
    }

    #[test]
    fn tx_shape_supports_base_append_and_remainder_outputs() {
        let mut fixture = CobuildTestFixture::new();
        let lock = deploy_protocol_dummy_script(fixture.context_mut(), 8, Vec::new());
        let base_input = live_resolved_facts(
            fixture.context_mut(),
            normal_output(lock.script.clone(), 1_000),
            Vec::new(),
        );
        let append_input = live_resolved_facts(
            fixture.context_mut(),
            normal_output(lock.script.clone(), 1_000),
            Vec::new(),
        );
        let base_output =
            TestCellOutput::new(normal_output(lock.script.clone(), 1_000), Vec::new());
        let append_output =
            TestCellOutput::new(normal_output(lock.script.clone(), 1_000), Vec::new());
        let remainder_output = TestCellOutput::new(normal_output(lock.script, 1_000), Vec::new());

        let mut shape = TxShape::new();
        shape.push_otx(OtxSpec {
            base_inputs: vec![base_input],
            base_outputs: vec![base_output],
            append_segments: vec![
                append_segment_spec(0x00)
                    .with_inputs(vec![append_input])
                    .with_outputs(vec![append_output]),
            ],
            ..Default::default()
        });
        shape.push_remainder_output(remainder_output);
        let built = shape.build();

        assert_eq!(built.tx.inputs().len(), 2);
        assert_eq!(built.tx.outputs().len(), 3);
        assert_eq!(built.tx.witnesses().len(), 2);
    }
}

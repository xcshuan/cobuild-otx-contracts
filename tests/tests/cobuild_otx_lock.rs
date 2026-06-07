use ckb_testtool::{
    ckb_types::{
        bytes::Bytes,
        core::{ScriptHashType, TransactionBuilder},
        packed::CellDep,
        prelude::*,
    },
    context::Context,
};
use cobuild_types::entity::{core::SighashAllOnly, witness::WitnessLayout};
use secp256k1::{PublicKey, Secp256k1, SecretKey};
use tests::{
    Loader,
    fixtures::{
        self,
        udt::{
            UdtTransferOtxParts, create_plain_locked_input, create_udt_input,
            signed_udt_transfer_otx, udt_output,
        },
    },
    framework::{
        scripts::packed_hash_to_array,
        signing::{sign_recoverable, tx_without_message_hash_for_inputs},
        tx::otx_start_witness,
    },
};

#[test]
fn contract_rejects_invalid_args() {
    let result = fixtures::invalid_args_case().verify();
    assert_lock_script_exit(result, 20);
}

#[test]
fn contract_rejects_without_relevant_signature_request() {
    let result = fixtures::no_relevant_signature_request_case().verify();
    assert_lock_script_exit(result, 40);
}

#[test]
fn contract_accepts_sighash_all_cobuild_signature() {
    let result = fixtures::signed_sighash_all_case().verify();
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn contract_accepts_sighash_all_when_current_lock_starts_after_input_zero() {
    let result = fixtures::signed_sighash_all_offset_lock_case().verify();
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn contract_accepts_otx_base_and_append_signatures() {
    let result = fixtures::signed_otx_dual_scope_case().verify();
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn contract_accepts_otx_signatures_covering_full_preimage_shape() {
    let result = fixtures::signed_otx_full_preimage_case().verify();
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn contract_accepts_two_udt_transfer_otxs_in_one_transaction() {
    let case = two_udt_transfer_otxs_case(false);
    assert_ne!(case.otx_a_lock_hash, case.otx_b_lock_hash);

    let result = case.context.verify_tx(&case.tx, 50_000_000);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn contract_accepts_two_udt_transfer_otxs_with_sighash_all_fee_input() {
    let case = two_udt_transfer_otxs_case(true);
    assert_ne!(case.otx_a_lock_hash, case.otx_b_lock_hash);
    assert_ne!(case.fee_lock_hash, Some(case.otx_a_lock_hash));
    assert_ne!(case.fee_lock_hash, Some(case.otx_b_lock_hash));

    let result = case.context.verify_tx(&case.tx, 50_000_000);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn contract_accepts_mixed_sighash_all_and_otx_signature_requests() {
    let result = fixtures::mixed_sighash_all_and_otx_case().verify();
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn contract_rejects_bad_seal() {
    let result = fixtures::bad_seal_case().verify();
    assert_lock_script_exit(result, 50);
}

#[test]
fn contract_rejects_malformed_cobuild_witness() {
    let result = fixtures::malformed_cobuild_witness_case().verify();
    assert_lock_script_exit(result, 30);
}

#[test]
fn contract_rejects_malformed_otx_layout() {
    let result = fixtures::malformed_otx_layout_case().verify();
    assert_lock_script_exit(result, 31);
}

fn assert_lock_script_exit(result: Result<u64, ckb_testtool::ckb_error::Error>, code: i8) {
    use ckb_testtool::{
        ckb_error::ErrorKind,
        ckb_script::{ScriptError, TransactionScriptError},
    };

    let err = result.expect_err("transaction must fail closed");
    assert_eq!(err.kind(), ErrorKind::Script);

    let script_error = err
        .root_cause()
        .downcast_ref::<TransactionScriptError>()
        .expect("script validation error");
    assert_eq!(
        script_error.originating_script().to_string(),
        "Inputs[0].Lock"
    );
    assert!(
        matches!(
            script_error.script_error(),
            ScriptError::ValidationFailure(_, actual) if *actual == code
        ),
        "{script_error:?}"
    );
}

struct TwoUdtTransferOtxsCase {
    context: Context,
    tx: ckb_testtool::ckb_types::core::TransactionView,
    fee_lock_hash: Option<[u8; 32]>,
    otx_a_lock_hash: [u8; 32],
    otx_b_lock_hash: [u8; 32],
}

fn two_udt_transfer_otxs_case(include_fee_input: bool) -> TwoUdtTransferOtxsCase {
    let secp = Secp256k1::new();
    let fee_secret_key = SecretKey::from_slice(&[1u8; 32]).expect("fixed secret key");
    let otx_a_secret_key = SecretKey::from_slice(&[2u8; 32]).expect("fixed secret key");
    let otx_b_secret_key = SecretKey::from_slice(&[3u8; 32]).expect("fixed secret key");

    let mut context = Context::default();
    let lock_bin = Loader::default().load_binary("cobuild-otx-lock");
    let lock_out_point = context.deploy_cell(lock_bin);
    let lock_dep = CellDep::new_builder()
        .out_point(lock_out_point.clone())
        .build();
    let fee_lock = context
        .build_script_with_hash_type(
            &lock_out_point,
            ScriptHashType::Data2,
            lock_args_for_key(&secp, &fee_secret_key).into(),
        )
        .expect("build fee cobuild-otx-lock script");
    let otx_a_lock = context
        .build_script_with_hash_type(
            &lock_out_point,
            ScriptHashType::Data2,
            lock_args_for_key(&secp, &otx_a_secret_key).into(),
        )
        .expect("build otx A cobuild-otx-lock script");
    let otx_b_lock = context
        .build_script_with_hash_type(
            &lock_out_point,
            ScriptHashType::Data2,
            lock_args_for_key(&secp, &otx_b_secret_key).into(),
        )
        .expect("build otx B cobuild-otx-lock script");
    let fee_lock_hash = packed_hash_to_array(fee_lock.calc_script_hash());
    let otx_a_lock_hash = packed_hash_to_array(otx_a_lock.calc_script_hash());
    let otx_b_lock_hash = packed_hash_to_array(otx_b_lock.calc_script_hash());
    let issuer_lock = context
        .build_script_with_hash_type(&lock_out_point, ScriptHashType::Data2, vec![0u8; 21].into())
        .expect("build cobuild-otx-lock script");
    let issuer_lock_hash = packed_hash_to_array(issuer_lock.calc_script_hash());

    let udt_bin = Loader::default().load_binary("test-udt");
    let udt_out_point = context.deploy_cell(udt_bin);
    let udt_dep = CellDep::new_builder()
        .out_point(udt_out_point.clone())
        .build();
    let udt_type = context
        .build_script_with_hash_type(
            &udt_out_point,
            ScriptHashType::Data2,
            Bytes::copy_from_slice(issuer_lock_hash.as_slice()),
        )
        .expect("build test-udt type script");

    let mut inputs = Vec::new();
    let mut input_parts = Vec::new();
    if include_fee_input {
        let (input, part) = create_plain_locked_input(
            &mut context,
            fee_lock.clone(),
            100_000_000_000,
            Bytes::new(),
        );
        inputs.push(input);
        input_parts.push(part);
    }

    let (otx_a_input, otx_a_part) =
        create_udt_input(&mut context, otx_a_lock.clone(), udt_type.clone(), 100);
    let (otx_b_input, otx_b_part) =
        create_udt_input(&mut context, otx_b_lock.clone(), udt_type.clone(), 300);
    inputs.push(otx_a_input);
    inputs.push(otx_b_input);
    input_parts.push(otx_a_part.clone());
    input_parts.push(otx_b_part.clone());

    let otx_a_outputs = vec![
        udt_output(otx_a_lock.clone(), udt_type.clone(), 40),
        udt_output(otx_a_lock.clone(), udt_type.clone(), 60),
    ];
    let otx_b_outputs = vec![
        udt_output(otx_b_lock.clone(), udt_type.clone(), 100),
        udt_output(otx_b_lock.clone(), udt_type.clone(), 100),
        udt_output(otx_b_lock.clone(), udt_type.clone(), 100),
    ];

    let mut builder = TransactionBuilder::default()
        .cell_dep(lock_dep)
        .cell_dep(udt_dep);
    for input in &inputs {
        builder = builder.input(input.clone());
    }
    for output in otx_a_outputs.iter().chain(otx_b_outputs.iter()) {
        builder = builder
            .output(output.cell.clone())
            .output_data(output.data.clone().pack());
    }
    let unsigned_tx = builder.build();

    let start_input = if include_fee_input { 1 } else { 0 };
    let otx_a = signed_udt_transfer_otx(
        otx_a_lock_hash,
        &UdtTransferOtxParts {
            start_input,
            start_output: 0,
            input: otx_a_part,
            outputs: otx_a_outputs,
        },
        &otx_a_secret_key,
    );
    let otx_b = signed_udt_transfer_otx(
        otx_b_lock_hash,
        &UdtTransferOtxParts {
            start_input: start_input + 1,
            start_output: 2,
            input: otx_b_part,
            outputs: otx_b_outputs,
        },
        &otx_b_secret_key,
    );

    let otx_start_witness_bytes = otx_start_witness(start_input as u32, 0, 2, 0);
    let otx_a_witness = Bytes::copy_from_slice(WitnessLayout::from(otx_a).as_slice());
    let otx_b_witness = Bytes::copy_from_slice(WitnessLayout::from(otx_b).as_slice());

    let mut witnesses = Vec::new();
    if include_fee_input {
        let signing_inputs: Vec<(&[u8], &[u8])> = input_parts
            .iter()
            .map(|part| (part.resolved_output.as_slice(), part.data.as_slice()))
            .collect();
        let witness_hash_inputs = vec![
            Vec::new(),
            otx_start_witness_bytes.to_vec(),
            otx_a_witness.to_vec(),
            otx_b_witness.to_vec(),
        ];
        let signing_message_hash = tx_without_message_hash_for_inputs(
            packed_hash_to_array(unsigned_tx.hash()),
            &signing_inputs,
            &witness_hash_inputs,
        );
        let tx_seal = sign_recoverable(&fee_secret_key, signing_message_hash);
        witnesses.push(
            Bytes::copy_from_slice(
                WitnessLayout::from(SighashAllOnly::new_builder().seal(tx_seal).build()).as_slice(),
            )
            .pack(),
        );
    }
    witnesses.push(otx_start_witness_bytes.pack());
    witnesses.push(otx_a_witness.pack());
    witnesses.push(otx_b_witness.pack());

    let tx = unsigned_tx
        .as_advanced_builder()
        .set_witnesses(witnesses)
        .build();

    TwoUdtTransferOtxsCase {
        context,
        tx,
        fee_lock_hash: include_fee_input.then_some(fee_lock_hash),
        otx_a_lock_hash,
        otx_b_lock_hash,
    }
}

fn lock_args_for_key(secp: &Secp256k1<secp256k1::All>, secret_key: &SecretKey) -> Vec<u8> {
    let public_key = PublicKey::from_secret_key(secp, secret_key);
    let public_key_hash = ckb_hash::blake2b_256(public_key.serialize());

    let mut args = vec![0u8];
    args.extend_from_slice(&public_key_hash[..20]);
    args
}

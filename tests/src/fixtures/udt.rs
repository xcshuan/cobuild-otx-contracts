use ckb_testtool::{
    ckb_types::{
        bytes::Bytes,
        core::{ScriptHashType, TransactionBuilder, TransactionView},
        packed::{CellDep, CellInput, CellOutput, Script},
        prelude::*,
    },
    context::Context,
};
use cobuild_types::entity::{
    core::{Otx, SighashAllOnly},
    witness::WitnessLayout,
};
use secp256k1::{PublicKey, Secp256k1, SecretKey};

use super::{
    otx_hash::otx_base_hash_with_base_output_start,
    support::{OtxFixtureInput, OtxFixtureOutputPart, OtxFixtureParts},
};
use crate::{
    Loader,
    framework::{
        cells::{
            TestCellOutput, TestResolvedInput, live_resolved_normal_input,
            live_resolved_typed_input,
        },
        cobuild::{empty_message, seal_pair},
        scripts::packed_hash_to_array,
        signing::{sign_recoverable, tx_without_message_hash_for_inputs},
        tx::otx_start_witness,
    },
};

pub struct TwoUdtTransferOtxsCase {
    pub context: Context,
    pub tx: TransactionView,
    pub fee_lock_hash: Option<[u8; 32]>,
    pub otx_a_lock_hash: [u8; 32],
    pub otx_b_lock_hash: [u8; 32],
}

pub struct UdtTransferOtxParts {
    pub start_input: usize,
    pub start_output: usize,
    pub input: TestResolvedInput,
    pub outputs: Vec<TestCellOutput>,
}

pub fn create_plain_locked_input(
    context: &mut Context,
    lock: Script,
    capacity: u64,
    data: Bytes,
) -> (CellInput, TestResolvedInput) {
    live_resolved_normal_input(context, lock, capacity, data)
}

pub fn create_udt_input(
    context: &mut Context,
    lock: Script,
    type_script: Script,
    amount: u128,
) -> (CellInput, TestResolvedInput) {
    live_resolved_typed_input(
        context,
        lock,
        type_script,
        100_000_000_000u64,
        Bytes::from(amount.to_le_bytes().to_vec()),
    )
}

pub fn udt_output(lock: Script, type_script: Script, amount: u128) -> TestCellOutput {
    TestCellOutput::new(
        CellOutput::new_builder()
            .capacity(90_000_000_000u64)
            .lock(lock)
            .type_(Some(type_script).pack())
            .build(),
        Bytes::from(amount.to_le_bytes().to_vec()),
    )
}

pub fn signed_udt_transfer_otx(
    lock_hash: [u8; 32],
    parts: &UdtTransferOtxParts,
    secret_key: &SecretKey,
) -> Otx {
    let base_outputs: Vec<OtxFixtureOutputPart> = parts
        .outputs
        .iter()
        .map(|output| OtxFixtureOutputPart {
            raw: output.cell.as_slice().to_vec(),
            data: output.data.to_vec(),
        })
        .collect();
    let hash_parts = OtxFixtureParts {
        start_input: parts.start_input,
        input_count: parts.start_input + 1,
        message: empty_message().as_slice().to_vec(),
        append_permissions: 0,
        base_input_masks: vec![0b0000_0011],
        base_inputs: vec![OtxFixtureInput {
            raw: parts.input.raw_input.clone(),
            resolved_output: parts.input.resolved_output.clone(),
            data: parts.input.data.clone(),
        }],
        append_inputs: Vec::new(),
        base_output_masks: full_output_masks(parts.outputs.len()),
        base_outputs,
        append_outputs: Vec::new(),
        base_cell_dep_masks: Vec::new(),
        base_cell_deps: Vec::new(),
        append_cell_deps: Vec::new(),
        base_header_dep_masks: Vec::new(),
        base_header_deps: Vec::new(),
        append_header_deps: Vec::new(),
    };
    let base_hash = otx_base_hash_with_base_output_start(&hash_parts, parts.start_output);
    let base_seal = sign_recoverable(secret_key, base_hash);
    let seals = vec![seal_pair(lock_hash, 0, base_seal)];

    Otx::new_builder()
        .message(empty_message())
        .append_permissions(0u8)
        .base_input_cells(1u32.to_le_bytes())
        .base_input_masks(vec![0b0000_0011])
        .base_output_cells((parts.outputs.len() as u32).to_le_bytes())
        .base_output_masks(full_output_masks(parts.outputs.len()))
        .base_cell_deps(0u32.to_le_bytes())
        .base_cell_dep_masks(Vec::<u8>::new())
        .base_header_deps(0u32.to_le_bytes())
        .base_header_dep_masks(Vec::<u8>::new())
        .append_input_cells(0u32.to_le_bytes())
        .append_output_cells(0u32.to_le_bytes())
        .append_cell_deps(0u32.to_le_bytes())
        .append_header_deps(0u32.to_le_bytes())
        .seals(seals)
        .build()
}

pub fn full_output_masks(output_count: usize) -> Vec<u8> {
    let bits = output_count * 4;
    let bytes = bits.div_ceil(8);
    let mut masks = vec![0xff; bytes];
    let extra_bits = bytes * 8 - bits;
    if extra_bits > 0 {
        let keep_bits = 8 - extra_bits;
        let last = masks.last_mut().expect("non-empty output mask");
        *last = (1u8 << keep_bits) - 1;
    }
    masks
}

pub fn two_udt_transfer_otxs_case(include_fee_input: bool) -> TwoUdtTransferOtxsCase {
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

use ckb_testtool::{
    ckb_types::{
        bytes::Bytes,
        core::TransactionBuilder,
        packed::{CellDep, CellInput, CellOutput},
        prelude::*,
    },
    context::Context,
};
use cobuild_types::entity::{
    core::{OtxStart, SighashAllOnly},
    witness::WitnessLayout,
};
use secp256k1::{PublicKey, Secp256k1, SecretKey};

use crate::framework::{
    cobuild::empty_message,
    scripts::packed_hash_to_array,
    signing::{sign_recoverable, tx_without_message_hash, tx_without_message_hash_for_inputs},
};

use super::{
    common::contracts::{deploy_always_success, deploy_cobuild_otx_lock},
    otx_hash::{otx_append_hash, otx_base_hash},
    support::*,
};

pub fn invalid_args_case() -> Case {
    build_case(Bytes::from(vec![0u8]))
}

pub fn no_relevant_signature_request_case() -> Case {
    let mut args = vec![0u8];
    args.extend_from_slice(&[1u8; 20]);
    build_case(Bytes::from(args))
}

pub fn signed_sighash_all_case() -> Case {
    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_slice(&[1u8; 32]).expect("fixed secret key");
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);
    let public_key_hash = ckb_hash::blake2b_256(public_key.serialize());

    let mut context = Context::default();
    let contract = deploy_cobuild_otx_lock(&mut context, 0, &public_key_hash[..20]);
    let contract_dep = contract.cell_dep;
    let lock = contract.script;
    let input_output = CellOutput::new_builder()
        .capacity(100_000_000_000u64)
        .lock(lock)
        .build();
    let input_out_point = context.create_cell(input_output.clone(), Bytes::new());
    let output = CellOutput::new_builder()
        .capacity(90_000_000_000u64)
        .lock(deploy_always_success(&mut context, Vec::new()).script)
        .build();
    let unsigned_tx = TransactionBuilder::default()
        .cell_dep(contract_dep)
        .input(
            CellInput::new_builder()
                .previous_output(input_out_point)
                .build(),
        )
        .output(output)
        .output_data(Bytes::new().pack())
        .witness(Bytes::new().pack())
        .build();

    let signing_message_hash = tx_without_message_hash(
        packed_hash_to_array(unsigned_tx.hash()),
        1,
        input_output.as_slice(),
        &[Vec::new()],
    );
    let seal = sign_recoverable(&secret_key, signing_message_hash);
    let witness = WitnessLayout::from(SighashAllOnly::new_builder().seal(seal).build());
    let tx = unsigned_tx
        .as_advanced_builder()
        .set_witnesses(vec![Bytes::copy_from_slice(witness.as_slice()).pack()])
        .build();

    Case { context, tx }
}

pub fn signed_sighash_all_offset_lock_case() -> Case {
    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_slice(&[1u8; 32]).expect("fixed secret key");
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);
    let public_key_hash = ckb_hash::blake2b_256(public_key.serialize());

    let mut context = Context::default();
    let contract = deploy_cobuild_otx_lock(&mut context, 0, &public_key_hash[..20]);
    let contract_dep = contract.cell_dep;
    let lock = contract.script;

    let always_success = deploy_always_success(&mut context, Vec::new());
    let always_success_dep = always_success.cell_dep;
    let other_lock = always_success.script;

    let other_input_output = CellOutput::new_builder()
        .capacity(100_000_000_000u64)
        .lock(other_lock)
        .build();
    let input_output = CellOutput::new_builder()
        .capacity(100_000_000_000u64)
        .lock(lock)
        .build();
    let other_input_out_point = context.create_cell(other_input_output.clone(), Bytes::new());
    let input_out_point = context.create_cell(input_output.clone(), Bytes::new());
    let output = CellOutput::new_builder()
        .capacity(90_000_000_000u64)
        .lock(deploy_always_success(&mut context, Vec::new()).script)
        .build();
    let unsigned_tx = TransactionBuilder::default()
        .cell_dep(contract_dep)
        .cell_dep(always_success_dep)
        .input(
            CellInput::new_builder()
                .previous_output(other_input_out_point)
                .build(),
        )
        .input(
            CellInput::new_builder()
                .previous_output(input_out_point)
                .build(),
        )
        .output(output)
        .output_data(Bytes::new().pack())
        .witness(Bytes::new().pack())
        .witness(Bytes::new().pack())
        .build();

    let signing_message_hash = tx_without_message_hash_for_inputs(
        packed_hash_to_array(unsigned_tx.hash()),
        &[
            (other_input_output.as_slice(), &[][..]),
            (input_output.as_slice(), &[][..]),
        ],
        &[Vec::new(), Vec::new()],
    );
    let seal = sign_recoverable(&secret_key, signing_message_hash);
    let witness = WitnessLayout::from(SighashAllOnly::new_builder().seal(seal).build());
    let tx = unsigned_tx
        .as_advanced_builder()
        .set_witnesses(vec![
            Bytes::new().pack(),
            Bytes::copy_from_slice(witness.as_slice()).pack(),
        ])
        .build();

    Case { context, tx }
}

pub fn signed_otx_dual_scope_case() -> Case {
    signed_otx_case(false, false)
}

pub fn signed_otx_full_preimage_case() -> Case {
    signed_otx_case_with_config(OtxCaseConfig {
        include_sighash_all: false,
        corrupt_append_seal: false,
        override_append_permissions: None,
        include_full_preimage: true,
    })
}

pub fn mixed_sighash_all_and_otx_case() -> Case {
    signed_otx_case(true, false)
}

pub fn bad_seal_case() -> Case {
    signed_otx_case(false, true)
}

pub fn malformed_cobuild_witness_case() -> Case {
    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_slice(&[1u8; 32]).expect("fixed secret key");
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);
    let public_key_hash = ckb_hash::blake2b_256(public_key.serialize());

    let mut args = vec![0u8];
    args.extend_from_slice(&public_key_hash[..20]);

    let mut case = build_case(Bytes::from(args));
    case.tx = case
        .tx
        .as_advanced_builder()
        .set_witnesses(vec![
            Bytes::from(malformed_sighash_all_only_witness()).pack(),
        ])
        .build();
    case
}

pub fn malformed_otx_layout_case() -> Case {
    signed_otx_case_with_options(false, false, Some(0x10))
}

fn signed_otx_case(include_sighash_all: bool, corrupt_append_seal: bool) -> Case {
    signed_otx_case_with_config(OtxCaseConfig {
        include_sighash_all,
        corrupt_append_seal,
        override_append_permissions: None,
        include_full_preimage: false,
    })
}

fn signed_otx_case_with_options(
    include_sighash_all: bool,
    corrupt_append_seal: bool,
    override_append_permissions: Option<u8>,
) -> Case {
    signed_otx_case_with_config(OtxCaseConfig {
        include_sighash_all,
        corrupt_append_seal,
        override_append_permissions,
        include_full_preimage: false,
    })
}

struct OtxCaseConfig {
    include_sighash_all: bool,
    corrupt_append_seal: bool,
    override_append_permissions: Option<u8>,
    include_full_preimage: bool,
}

fn signed_otx_case_with_config(config: OtxCaseConfig) -> Case {
    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_slice(&[1u8; 32]).expect("fixed secret key");
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);
    let public_key_hash = ckb_hash::blake2b_256(public_key.serialize());

    let mut context = Context::default();
    let contract = deploy_cobuild_otx_lock(&mut context, 0, &public_key_hash[..20]);
    let script_hash = contract.script_hash;
    let contract_dep = contract.cell_dep;
    let lock = contract.script;

    let input_output = CellOutput::new_builder()
        .capacity(100_000_000_000u64)
        .lock(lock)
        .build();
    let input_count = if config.include_sighash_all { 3 } else { 2 };
    let mut input_out_points = Vec::with_capacity(input_count);
    for _ in 0..input_count {
        input_out_points.push(context.create_cell(input_output.clone(), Bytes::new()));
    }
    let cell_inputs: Vec<CellInput> = input_out_points
        .into_iter()
        .map(|previous_output| {
            CellInput::new_builder()
                .previous_output(previous_output)
                .build()
        })
        .collect();
    let mut builder = TransactionBuilder::default().cell_dep(contract_dep);
    for input in &cell_inputs {
        builder = builder.input(input.clone());
    }
    let mut base_cell_deps = Vec::new();
    let mut append_cell_deps = Vec::new();
    let mut base_header_deps = Vec::new();
    let mut append_header_deps = Vec::new();
    let outputs = if config.include_full_preimage {
        let base_cell_dep = CellDep::new_builder()
            .out_point(context.deploy_cell(Bytes::from(vec![0x51])))
            .build();
        let append_cell_dep = CellDep::new_builder()
            .out_point(context.deploy_cell(Bytes::from(vec![0x52])))
            .build();
        base_cell_deps.push(base_cell_dep.as_slice().to_vec());
        append_cell_deps.push(append_cell_dep.as_slice().to_vec());
        base_header_deps.push([0x61u8; 32]);
        append_header_deps.push([0x62u8; 32]);
        builder = builder
            .cell_dep(base_cell_dep.clone())
            .cell_dep(append_cell_dep.clone())
            .header_dep([0x61u8; 32].pack())
            .header_dep([0x62u8; 32].pack());

        vec![
            OtxFixtureOutput {
                cell: CellOutput::new_builder()
                    .capacity(91_000_000_000u64)
                    .lock(deploy_always_success(&mut context, Vec::new()).script)
                    .build(),
                data: vec![0x71, 0x72],
            },
            OtxFixtureOutput {
                cell: CellOutput::new_builder()
                    .capacity(92_000_000_000u64)
                    .lock(deploy_always_success(&mut context, Vec::new()).script)
                    .build(),
                data: vec![0x81, 0x82, 0x83],
            },
        ]
    } else {
        vec![OtxFixtureOutput {
            cell: CellOutput::new_builder()
                .capacity(90_000_000_000u64)
                .lock(deploy_always_success(&mut context, Vec::new()).script)
                .build(),
            data: Vec::new(),
        }]
    };

    for output in &outputs {
        builder = builder
            .output(output.cell.clone())
            .output_data(Bytes::from(output.data.clone()).pack());
    }
    let unsigned_tx = builder.witness(Bytes::new().pack()).build();

    let start_input = if config.include_sighash_all { 1 } else { 0 };
    let (base_outputs, append_outputs, base_output_masks) = if config.include_full_preimage {
        (
            vec![OtxFixtureOutputPart {
                raw: outputs[0].cell.as_slice().to_vec(),
                data: outputs[0].data.clone(),
            }],
            vec![OtxFixtureOutputPart {
                raw: outputs[1].cell.as_slice().to_vec(),
                data: outputs[1].data.clone(),
            }],
            vec![0b0000_1111],
        )
    } else {
        (Vec::new(), Vec::new(), Vec::new())
    };
    let default_append_permissions = if config.include_full_preimage {
        0x0f
    } else {
        0x01
    };
    let otx_parts = OtxFixtureParts {
        start_input,
        input_count,
        message: empty_message().as_slice().to_vec(),
        append_permissions: config
            .override_append_permissions
            .unwrap_or(default_append_permissions),
        base_input_masks: vec![0b0000_0011],
        base_inputs: vec![OtxFixtureInput {
            raw: cell_inputs[start_input].as_slice().to_vec(),
            resolved_output: input_output.as_slice().to_vec(),
            data: Vec::new(),
        }],
        append_inputs: vec![OtxFixtureInput {
            raw: cell_inputs[start_input + 1].as_slice().to_vec(),
            resolved_output: input_output.as_slice().to_vec(),
            data: Vec::new(),
        }],
        base_output_masks,
        base_outputs,
        append_outputs,
        base_cell_dep_masks: if config.include_full_preimage {
            vec![0b0000_0001]
        } else {
            Vec::new()
        },
        base_cell_deps,
        append_cell_deps,
        base_header_dep_masks: if config.include_full_preimage {
            vec![0b0000_0001]
        } else {
            Vec::new()
        },
        base_header_deps,
        append_header_deps,
    };
    let base_hash = otx_base_hash(&otx_parts);
    let append_hash = otx_append_hash(&otx_parts, base_hash);
    let base_seal = sign_recoverable(&secret_key, base_hash);
    let mut append_seal = sign_recoverable(&secret_key, append_hash);
    if config.corrupt_append_seal {
        append_seal[0] ^= 0x01;
    }

    let otx_start = WitnessLayout::from(
        OtxStart::new_builder()
            .start_input_cell((start_input as u32).to_le_bytes())
            .start_output_cell(0u32.to_le_bytes())
            .start_cell_deps(1u32.to_le_bytes())
            .start_header_deps(0u32.to_le_bytes())
            .build(),
    );
    let otx = WitnessLayout::from(otx_witness(script_hash, &otx_parts, base_seal, append_seal));
    let otx_start_witness = Bytes::copy_from_slice(otx_start.as_slice());
    let otx_witness = Bytes::copy_from_slice(otx.as_slice());

    let mut witnesses = Vec::new();
    if config.include_sighash_all {
        let mut signing_witnesses = vec![Vec::new(); input_count];
        signing_witnesses.push(otx_start_witness.to_vec());
        signing_witnesses.push(otx_witness.to_vec());
        let signing_message_hash = tx_without_message_hash(
            packed_hash_to_array(unsigned_tx.hash()),
            input_count,
            input_output.as_slice(),
            &signing_witnesses,
        );
        let tx_seal = sign_recoverable(&secret_key, signing_message_hash);
        witnesses.push(
            Bytes::copy_from_slice(
                WitnessLayout::from(SighashAllOnly::new_builder().seal(tx_seal).build()).as_slice(),
            )
            .pack(),
        );
        witnesses.push(Bytes::new().pack());
        witnesses.push(Bytes::new().pack());
    }
    witnesses.push(otx_start_witness.pack());
    witnesses.push(otx_witness.pack());

    let tx = unsigned_tx
        .as_advanced_builder()
        .set_witnesses(witnesses)
        .build();

    Case { context, tx }
}

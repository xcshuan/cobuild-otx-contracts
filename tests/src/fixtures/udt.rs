use ckb_testtool::{
    ckb_types::{
        bytes::Bytes,
        packed::{CellInput, CellOutput, Script},
        prelude::*,
    },
    context::Context,
};
use cobuild_types::entity::core::Otx;
use secp256k1::SecretKey;

use super::{
    otx_hash::otx_base_hash_with_base_output_start,
    support::{OtxFixtureInput, OtxFixtureOutputPart, OtxFixtureParts},
};
use crate::framework::{
    cells::{
        TestCellOutput, TestResolvedInput, live_resolved_normal_input, live_resolved_typed_input,
    },
    cobuild::{empty_message, seal_pair},
    signing::sign_recoverable,
};

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

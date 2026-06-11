use ckb_testtool::{
    ckb_error::Error,
    ckb_types::{
        bytes::Bytes,
        core::{Cycle, TransactionBuilder, TransactionView},
        packed::{CellInput, CellOutput},
        prelude::*,
    },
    context::Context,
};
use cobuild_types::entity::core::Otx;

use crate::framework::cobuild::{empty_message, seal_pair};

use super::common::contracts::{deploy_always_success, deploy_cobuild_otx_lock_code};

pub struct Case {
    pub(super) context: Context,
    pub(super) tx: TransactionView,
}

impl Case {
    pub fn verify(self) -> Result<Cycle, Error> {
        self.context.verify_tx(&self.tx, 50_000_000)
    }
}
#[derive(Clone)]
pub(super) struct OtxFixtureInput {
    pub(super) raw: Vec<u8>,
    pub(super) resolved_output: Vec<u8>,
    pub(super) data: Vec<u8>,
}

#[derive(Clone)]
pub(super) struct OtxFixtureOutput {
    pub(super) cell: CellOutput,
    pub(super) data: Vec<u8>,
}

#[derive(Clone)]
pub(super) struct OtxFixtureOutputPart {
    pub(super) raw: Vec<u8>,
    pub(super) data: Vec<u8>,
}

// Input model for the cobuild-otx-lock hash oracle. This stays out of
// framework because it encodes lock-verification preimage details.
pub(super) struct OtxFixtureParts {
    pub(super) start_input: usize,
    pub(super) input_count: usize,
    pub(super) message: Vec<u8>,
    pub(super) append_permissions: u8,
    pub(super) base_input_masks: Vec<u8>,
    pub(super) base_inputs: Vec<OtxFixtureInput>,
    pub(super) append_inputs: Vec<OtxFixtureInput>,
    pub(super) base_output_masks: Vec<u8>,
    pub(super) base_outputs: Vec<OtxFixtureOutputPart>,
    pub(super) append_outputs: Vec<OtxFixtureOutputPart>,
    pub(super) base_cell_dep_masks: Vec<u8>,
    pub(super) base_cell_deps: Vec<Vec<u8>>,
    pub(super) append_cell_deps: Vec<Vec<u8>>,
    pub(super) base_header_dep_masks: Vec<u8>,
    pub(super) base_header_deps: Vec<[u8; 32]>,
    pub(super) append_header_deps: Vec<[u8; 32]>,
}

pub(super) struct OtxHashFixture {
    pub(super) raw_inputs: Vec<Vec<u8>>,
    pub(super) resolved_outputs: Vec<Vec<u8>>,
    pub(super) resolved_data: Vec<Vec<u8>>,
    pub(super) raw_outputs: Vec<Vec<u8>>,
    pub(super) output_data: Vec<Vec<u8>>,
    pub(super) raw_cell_deps: Vec<Vec<u8>>,
    pub(super) header_deps: Vec<[u8; 32]>,
}

pub(super) fn otx_witness(
    script_hash: [u8; 32],
    parts: &OtxFixtureParts,
    base_seal: Vec<u8>,
    append_seal: Vec<u8>,
) -> Otx {
    let message = empty_message();
    let seals = vec![
        seal_pair(script_hash, 0, base_seal),
        seal_pair(script_hash, 1, append_seal),
    ];
    Otx::new_builder()
        .message(message)
        .append_permissions(parts.append_permissions)
        .base_input_cells((parts.base_inputs.len() as u32).to_le_bytes())
        .base_input_masks(parts.base_input_masks.clone())
        .base_output_cells((parts.base_outputs.len() as u32).to_le_bytes())
        .base_output_masks(parts.base_output_masks.clone())
        .base_cell_deps((parts.base_cell_deps.len() as u32).to_le_bytes())
        .base_cell_dep_masks(parts.base_cell_dep_masks.clone())
        .base_header_deps((parts.base_header_deps.len() as u32).to_le_bytes())
        .base_header_dep_masks(parts.base_header_dep_masks.clone())
        .append_input_cells((parts.append_inputs.len() as u32).to_le_bytes())
        .append_output_cells((parts.append_outputs.len() as u32).to_le_bytes())
        .append_cell_deps((parts.append_cell_deps.len() as u32).to_le_bytes())
        .append_header_deps((parts.append_header_deps.len() as u32).to_le_bytes())
        .seals(seals)
        .build()
}

pub(super) fn build_case(args: Bytes) -> Case {
    let mut context = Context::default();
    let contract = deploy_cobuild_otx_lock_code(&mut context, args.to_vec());
    let input_out_point = context.create_cell(
        CellOutput::new_builder()
            .capacity(100_000_000_000u64)
            .lock(contract.script)
            .build(),
        Bytes::new(),
    );
    let output = CellOutput::new_builder()
        .capacity(90_000_000_000u64)
        .lock(deploy_always_success(&mut context, Vec::new()).script)
        .build();
    let tx = TransactionBuilder::default()
        .cell_dep(contract.cell_dep)
        .input(
            CellInput::new_builder()
                .previous_output(input_out_point)
                .build(),
        )
        .output(output)
        .output_data(Bytes::new().pack())
        .witness(Bytes::new().pack())
        .build();
    Case { context, tx }
}

pub(super) fn malformed_sighash_all_only_witness() -> Vec<u8> {
    witness_union(0xff00_0002, &table(&[Vec::new()]))
}

fn witness_union(item_id: u32, item: &[u8]) -> Vec<u8> {
    let mut witness = Vec::with_capacity(4 + item.len());
    witness.extend_from_slice(&item_id.to_le_bytes());
    witness.extend_from_slice(item);
    witness
}

fn table(fields: &[Vec<u8>]) -> Vec<u8> {
    let header_size = 4 + fields.len() * 4;
    let total_size = header_size + fields.iter().map(Vec::len).sum::<usize>();
    let mut out = Vec::with_capacity(total_size);
    out.extend_from_slice(&(total_size as u32).to_le_bytes());
    let mut offset = header_size as u32;
    for field in fields {
        out.extend_from_slice(&offset.to_le_bytes());
        offset += field.len() as u32;
    }
    for field in fields {
        out.extend_from_slice(field);
    }
    out
}

use std::ops::Range;

use ckb_testtool::ckb_types::{
    bytes::Bytes,
    core::{TransactionBuilder, TransactionView},
    packed::CellDep,
    prelude::*,
};
use cobuild_types::entity::{
    core::{Message as CobuildMessage, SealPair, SighashAll},
    witness::WitnessLayout,
};

use crate::framework::{
    cells::{ResolvedInputFacts, TestCellOutput},
    cobuild::OtxBuilder,
};

use super::{
    handles::{
        CellDepHandle, EntityIndexMap, HeaderDepHandle, InputHandle, OtxHandle, OutputHandle,
        WitnessHandle,
    },
    otx_start_witness,
};

#[derive(Clone, Debug, Default)]
pub struct OtxSegment {
    pub message: Option<CobuildMessage>,
    pub base_inputs: Vec<ResolvedInputFacts>,
    pub append_inputs: Vec<ResolvedInputFacts>,
    pub base_outputs: Vec<TestCellOutput>,
    pub append_outputs: Vec<TestCellOutput>,
    pub base_cell_deps: Vec<CellDep>,
    pub append_cell_deps: Vec<CellDep>,
    pub base_header_deps: Vec<[u8; 32]>,
    pub append_header_deps: Vec<[u8; 32]>,
    pub base_input_masks: Option<Vec<u8>>,
    pub base_output_masks: Option<Vec<u8>>,
    pub base_cell_dep_masks: Option<Vec<u8>>,
    pub base_header_dep_masks: Option<Vec<u8>>,
    pub seals: Vec<SealPair>,
}

#[derive(Clone, Debug)]
pub struct OtxRangeFacts {
    pub otx: OtxHandle,
    pub base_inputs: Range<usize>,
    pub append_inputs: Range<usize>,
    pub base_outputs: Range<usize>,
    pub append_outputs: Range<usize>,
    pub base_cell_deps: Range<usize>,
    pub append_cell_deps: Range<usize>,
    pub base_header_deps: Range<usize>,
    pub append_header_deps: Range<usize>,
}

#[derive(Clone, Debug)]
pub struct BuiltTxShape {
    pub tx: TransactionView,
    pub inputs: EntityIndexMap<InputHandle>,
    pub outputs: EntityIndexMap<OutputHandle>,
    pub witnesses: EntityIndexMap<WitnessHandle>,
    pub cell_deps: EntityIndexMap<CellDepHandle>,
    pub header_deps: EntityIndexMap<HeaderDepHandle>,
    pub resolved_inputs: Vec<ResolvedInputFacts>,
    pub otx_ranges: Vec<OtxRangeFacts>,
    otx_witness_start: usize,
}

#[derive(Clone, Debug, Default)]
pub struct TxShape {
    prefix_inputs: Vec<(InputHandle, ResolvedInputFacts)>,
    prefix_cell_deps: Vec<(CellDepHandle, CellDep)>,
    otxs: Vec<TrackedOtxSegment>,
    remainder_outputs: Vec<(OutputHandle, TestCellOutput)>,
    tx_level_message: Option<CobuildMessage>,
    next_input: usize,
    next_output: usize,
    next_cell_dep: usize,
    next_header_dep: usize,
}

#[derive(Clone, Debug)]
struct TrackedOtxSegment {
    handle: OtxHandle,
    segment: OtxSegment,
    base_input_handles: Vec<InputHandle>,
    append_input_handles: Vec<InputHandle>,
    base_output_handles: Vec<OutputHandle>,
    append_output_handles: Vec<OutputHandle>,
    base_cell_dep_handles: Vec<CellDepHandle>,
    append_cell_dep_handles: Vec<CellDepHandle>,
    base_header_dep_handles: Vec<HeaderDepHandle>,
    append_header_dep_handles: Vec<HeaderDepHandle>,
}

impl TxShape {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_prefix_input(&mut self, input: ResolvedInputFacts) -> InputHandle {
        let handle = self.next_input_handle();
        self.prefix_inputs.push((handle, input));
        handle
    }

    pub fn push_prefix_cell_dep(&mut self, cell_dep: CellDep) -> CellDepHandle {
        let handle = self.next_cell_dep_handle();
        self.prefix_cell_deps.push((handle, cell_dep));
        handle
    }

    pub fn push_otx(&mut self, segment: OtxSegment) -> OtxHandle {
        assert!(
            !segment.base_inputs.is_empty(),
            "OTX segment requires non-zero base inputs"
        );

        let handle = OtxHandle::from_raw(self.otxs.len());
        let base_input_handles = self.input_handles(segment.base_inputs.len());
        let append_input_handles = self.input_handles(segment.append_inputs.len());
        let base_output_handles = self.output_handles(segment.base_outputs.len());
        let append_output_handles = self.output_handles(segment.append_outputs.len());
        let base_cell_dep_handles = self.cell_dep_handles(segment.base_cell_deps.len());
        let append_cell_dep_handles = self.cell_dep_handles(segment.append_cell_deps.len());
        let base_header_dep_handles = self.header_dep_handles(segment.base_header_deps.len());
        let append_header_dep_handles = self.header_dep_handles(segment.append_header_deps.len());

        self.otxs.push(TrackedOtxSegment {
            handle,
            segment,
            base_input_handles,
            append_input_handles,
            base_output_handles,
            append_output_handles,
            base_cell_dep_handles,
            append_cell_dep_handles,
            base_header_dep_handles,
            append_header_dep_handles,
        });

        handle
    }

    pub fn push_remainder_output(&mut self, output: TestCellOutput) -> OutputHandle {
        let handle = self.next_output_handle();
        self.remainder_outputs.push((handle, output));
        handle
    }

    pub fn tx_level_message(&mut self, message: CobuildMessage) {
        self.tx_level_message = Some(message);
    }

    pub fn otx_append_output(&self, otx: OtxHandle, local_index: usize) -> OutputHandle {
        self.otx(otx).append_output_handles[local_index]
    }

    pub fn otx_base_output(&self, otx: OtxHandle, local_index: usize) -> OutputHandle {
        self.otx(otx).base_output_handles[local_index]
    }

    pub fn otx_base_input(&self, otx: OtxHandle, local_index: usize) -> InputHandle {
        self.otx(otx).base_input_handles[local_index]
    }

    pub fn otx_append_input(&self, otx: OtxHandle, local_index: usize) -> InputHandle {
        self.otx(otx).append_input_handles[local_index]
    }

    pub fn otx_base_cell_dep(&self, otx: OtxHandle, local_index: usize) -> CellDepHandle {
        self.otx(otx).base_cell_dep_handles[local_index]
    }

    pub fn otx_append_cell_dep(&self, otx: OtxHandle, local_index: usize) -> CellDepHandle {
        self.otx(otx).append_cell_dep_handles[local_index]
    }

    pub fn otx_base_header_dep(&self, otx: OtxHandle, local_index: usize) -> HeaderDepHandle {
        self.otx(otx).base_header_dep_handles[local_index]
    }

    pub fn otx_append_header_dep(&self, otx: OtxHandle, local_index: usize) -> HeaderDepHandle {
        self.otx(otx).append_header_dep_handles[local_index]
    }

    pub fn build(self) -> BuiltTxShape {
        let mut builder = TransactionBuilder::default();
        let mut inputs = EntityIndexMap::default();
        let mut outputs = EntityIndexMap::default();
        let mut witnesses = EntityIndexMap::default();
        let mut cell_deps = EntityIndexMap::default();
        let mut header_deps = EntityIndexMap::default();
        let mut resolved_inputs = Vec::new();
        let mut otx_ranges: Vec<_> = self
            .otxs
            .iter()
            .map(|otx| OtxRangeFacts {
                otx: otx.handle,
                base_inputs: 0..0,
                append_inputs: 0..0,
                base_outputs: 0..0,
                append_outputs: 0..0,
                base_cell_deps: 0..0,
                append_cell_deps: 0..0,
                base_header_deps: 0..0,
                append_header_deps: 0..0,
            })
            .collect();
        let mut cell_dep_cursor = 0;
        let mut header_dep_cursor = 0;

        let start_cell_deps = self.prefix_cell_deps.len();
        for (handle, dep) in &self.prefix_cell_deps {
            cell_deps.insert(*handle, cell_dep_cursor);
            builder = builder.cell_dep(dep.clone());
            cell_dep_cursor += 1;
        }

        for (range_index, otx) in self.otxs.iter().enumerate() {
            let base_start = cell_dep_cursor;
            for (handle, dep) in otx
                .base_cell_dep_handles
                .iter()
                .copied()
                .zip(otx.segment.base_cell_deps.iter())
            {
                cell_deps.insert(handle, cell_dep_cursor);
                builder = builder.cell_dep(dep.clone());
                cell_dep_cursor += 1;
            }
            otx_ranges[range_index].base_cell_deps = base_start..cell_dep_cursor;

            let append_start = cell_dep_cursor;
            for (handle, dep) in otx
                .append_cell_dep_handles
                .iter()
                .copied()
                .zip(otx.segment.append_cell_deps.iter())
            {
                cell_deps.insert(handle, cell_dep_cursor);
                builder = builder.cell_dep(dep.clone());
                cell_dep_cursor += 1;
            }
            otx_ranges[range_index].append_cell_deps = append_start..cell_dep_cursor;
        }

        for (range_index, otx) in self.otxs.iter().enumerate() {
            let base_start = header_dep_cursor;
            for (handle, dep) in otx
                .base_header_dep_handles
                .iter()
                .copied()
                .zip(otx.segment.base_header_deps.iter())
            {
                header_deps.insert(handle, header_dep_cursor);
                builder = builder.header_dep(dep.pack());
                header_dep_cursor += 1;
            }
            otx_ranges[range_index].base_header_deps = base_start..header_dep_cursor;

            let append_start = header_dep_cursor;
            for (handle, dep) in otx
                .append_header_dep_handles
                .iter()
                .copied()
                .zip(otx.segment.append_header_deps.iter())
            {
                header_deps.insert(handle, header_dep_cursor);
                builder = builder.header_dep(dep.pack());
                header_dep_cursor += 1;
            }
            otx_ranges[range_index].append_header_deps = append_start..header_dep_cursor;
        }

        for (handle, input) in self.prefix_inputs {
            inputs.insert(handle, resolved_inputs.len());
            builder = builder.input(input.input.clone());
            resolved_inputs.push(input);
        }
        for (range_index, otx) in self.otxs.iter().enumerate() {
            let base_start = resolved_inputs.len();
            for (handle, input) in otx
                .base_input_handles
                .iter()
                .copied()
                .zip(otx.segment.base_inputs.iter())
            {
                inputs.insert(handle, resolved_inputs.len());
                builder = builder.input(input.input.clone());
                resolved_inputs.push(input.clone());
            }
            otx_ranges[range_index].base_inputs = base_start..resolved_inputs.len();

            let append_start = resolved_inputs.len();
            for (handle, input) in otx
                .append_input_handles
                .iter()
                .copied()
                .zip(otx.segment.append_inputs.iter())
            {
                inputs.insert(handle, resolved_inputs.len());
                builder = builder.input(input.input.clone());
                resolved_inputs.push(input.clone());
            }
            otx_ranges[range_index].append_inputs = append_start..resolved_inputs.len();
        }

        let mut output_cursor = 0;
        for (range_index, otx) in self.otxs.iter().enumerate() {
            let base_start = output_cursor;
            for (handle, output) in otx
                .base_output_handles
                .iter()
                .copied()
                .zip(otx.segment.base_outputs.iter())
            {
                outputs.insert(handle, output_cursor);
                builder = builder
                    .output(output.cell.clone())
                    .output_data(output.data.clone().pack());
                output_cursor += 1;
            }
            otx_ranges[range_index].base_outputs = base_start..output_cursor;

            let append_start = output_cursor;
            for (handle, output) in otx
                .append_output_handles
                .iter()
                .copied()
                .zip(otx.segment.append_outputs.iter())
            {
                outputs.insert(handle, output_cursor);
                builder = builder
                    .output(output.cell.clone())
                    .output_data(output.data.clone().pack());
                output_cursor += 1;
            }
            otx_ranges[range_index].append_outputs = append_start..output_cursor;
        }

        for (handle, output) in self.remainder_outputs {
            outputs.insert(handle, output_cursor);
            builder = builder.output(output.cell).output_data(output.data.pack());
            output_cursor += 1;
        }

        let mut witness_cursor = 0;
        if let Some(message) = self.tx_level_message {
            let witness = WitnessLayout::from(
                SighashAll::new_builder()
                    .seal(Vec::<u8>::new())
                    .message(message)
                    .build(),
            );
            witnesses.insert(WitnessHandle::from_raw(witness_cursor), witness_cursor);
            builder = builder.witness(Bytes::copy_from_slice(witness.as_slice()).pack());
            witness_cursor += 1;
        }
        if !self.otxs.is_empty() {
            let start_input_cell = resolved_inputs
                .len()
                .saturating_sub(total_otx_inputs(&self.otxs))
                as u32;
            witnesses.insert(WitnessHandle::from_raw(witness_cursor), witness_cursor);
            builder = builder
                .witness(otx_start_witness(start_input_cell, 0, start_cell_deps as u32, 0).pack());
            witness_cursor += 1;
        }
        let otx_witness_start = witness_cursor;
        for (otx_index, otx) in self.otxs.iter().enumerate() {
            let mut builder_for_otx = OtxBuilder::new()
                .base_input_cells(otx.segment.base_inputs.len() as u32)
                .base_output_cells(otx.segment.base_outputs.len() as u32)
                .base_cell_deps(otx.segment.base_cell_deps.len() as u32)
                .base_header_deps(otx.segment.base_header_deps.len() as u32)
                .append_input_cells(otx.segment.append_inputs.len() as u32)
                .append_output_cells(otx.segment.append_outputs.len() as u32)
                .append_cell_deps(otx.segment.append_cell_deps.len() as u32)
                .append_header_deps(otx.segment.append_header_deps.len() as u32);
            if !otx.segment.append_inputs.is_empty() {
                builder_for_otx = builder_for_otx.allow_append_inputs();
            }
            if !otx.segment.append_outputs.is_empty() {
                builder_for_otx = builder_for_otx.allow_append_outputs();
            }
            if !otx.segment.append_cell_deps.is_empty() {
                builder_for_otx = builder_for_otx.allow_append_cell_deps();
            }
            if !otx.segment.append_header_deps.is_empty() {
                builder_for_otx = builder_for_otx.allow_append_header_deps();
            }
            if let Some(masks) = &otx.segment.base_input_masks {
                builder_for_otx = builder_for_otx.base_input_masks_raw(masks.clone());
            }
            if let Some(masks) = &otx.segment.base_output_masks {
                builder_for_otx = builder_for_otx.base_output_masks_raw(masks.clone());
            }
            if let Some(masks) = &otx.segment.base_cell_dep_masks {
                builder_for_otx = builder_for_otx.base_cell_dep_masks_raw(masks.clone());
            }
            if let Some(masks) = &otx.segment.base_header_dep_masks {
                builder_for_otx = builder_for_otx.base_header_dep_masks_raw(masks.clone());
            }
            if let Some(message) = &otx.segment.message {
                builder_for_otx = builder_for_otx.message(message.clone());
            }
            builder_for_otx = builder_for_otx.seals(otx.segment.seals.clone());

            let otx = builder_for_otx.build();
            let witness = WitnessLayout::from(otx);
            let witness_index = witness_cursor + otx_index;
            witnesses.insert(WitnessHandle::from_raw(witness_index), witness_index);
            builder = builder.witness(Bytes::copy_from_slice(witness.as_slice()).pack());
        }

        BuiltTxShape {
            tx: builder.build(),
            inputs,
            outputs,
            witnesses,
            cell_deps,
            header_deps,
            resolved_inputs,
            otx_ranges,
            otx_witness_start,
        }
    }

    fn otx(&self, otx: OtxHandle) -> &TrackedOtxSegment {
        &self.otxs[otx.0]
    }

    fn input_handles(&mut self, count: usize) -> Vec<InputHandle> {
        (0..count).map(|_| self.next_input_handle()).collect()
    }

    fn output_handles(&mut self, count: usize) -> Vec<OutputHandle> {
        (0..count).map(|_| self.next_output_handle()).collect()
    }

    fn cell_dep_handles(&mut self, count: usize) -> Vec<CellDepHandle> {
        (0..count).map(|_| self.next_cell_dep_handle()).collect()
    }

    fn header_dep_handles(&mut self, count: usize) -> Vec<HeaderDepHandle> {
        (0..count).map(|_| self.next_header_dep_handle()).collect()
    }

    fn next_input_handle(&mut self) -> InputHandle {
        let handle = InputHandle::from_raw(self.next_input);
        self.next_input += 1;
        handle
    }

    fn next_output_handle(&mut self) -> OutputHandle {
        let handle = OutputHandle::from_raw(self.next_output);
        self.next_output += 1;
        handle
    }

    fn next_cell_dep_handle(&mut self) -> CellDepHandle {
        let handle = CellDepHandle::from_raw(self.next_cell_dep);
        self.next_cell_dep += 1;
        handle
    }

    fn next_header_dep_handle(&mut self) -> HeaderDepHandle {
        let handle = HeaderDepHandle::from_raw(self.next_header_dep);
        self.next_header_dep += 1;
        handle
    }
}

fn total_otx_inputs(otxs: &[TrackedOtxSegment]) -> usize {
    otxs.iter()
        .map(|otx| otx.segment.base_inputs.len() + otx.segment.append_inputs.len())
        .sum()
}

impl BuiltTxShape {
    pub fn tx_level_witness(&self) -> WitnessHandle {
        self.witnesses
            .handle_at_tx_index(0)
            .expect("transaction shape has no tx-level witness")
    }

    pub fn otx_start_witness(&self) -> WitnessHandle {
        assert!(
            self.otx_witness_start > 0,
            "transaction shape has no OTX start witness"
        );
        WitnessHandle::from_raw(self.otx_witness_start - 1)
    }

    pub fn otx_witness(&self, otx: OtxHandle) -> WitnessHandle {
        WitnessHandle::from_raw(self.otx_witness_start + otx.0)
    }
}

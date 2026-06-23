use std::ops::Range;

use ckb_testtool::ckb_types::{
    bytes::Bytes,
    core::{TransactionBuilder, TransactionView},
    packed::CellDep,
    prelude::*,
};
use cobuild_core::protocol::{
    APPEND_PERMISSION_CELL_DEPS_BIT, APPEND_PERMISSION_HEADER_DEPS_BIT,
    APPEND_PERMISSION_INPUTS_BIT, APPEND_PERMISSION_OUTPUTS_BIT,
};
use cobuild_types::entity::{
    core::{LockSeal, Message as CobuildMessage, SighashAll},
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppendPermissionsSpec {
    Auto,
    Explicit(u8),
}

impl Default for AppendPermissionsSpec {
    fn default() -> Self {
        Self::Auto
    }
}

impl AppendPermissionsSpec {
    pub fn new() -> Self {
        Self::Explicit(0)
    }

    pub fn none() -> Self {
        Self::new()
    }

    pub fn raw(value: u8) -> Self {
        Self::Explicit(value)
    }

    pub fn allow_inputs(self) -> Self {
        self.allow(APPEND_PERMISSION_INPUTS_BIT)
    }

    pub fn allow_outputs(self) -> Self {
        self.allow(APPEND_PERMISSION_OUTPUTS_BIT)
    }

    pub fn allow_cell_deps(self) -> Self {
        self.allow(APPEND_PERMISSION_CELL_DEPS_BIT)
    }

    pub fn allow_header_deps(self) -> Self {
        self.allow(APPEND_PERMISSION_HEADER_DEPS_BIT)
    }

    fn allow(self, bit: u8) -> Self {
        let raw = match self {
            Self::Auto => 0,
            Self::Explicit(raw) => raw,
        };
        Self::Explicit(raw | (1 << bit))
    }
}

#[derive(Clone, Debug, Default)]
pub struct OtxSpec {
    pub message: Option<CobuildMessage>,
    pub base_inputs: Vec<ResolvedInputFacts>,
    pub base_outputs: Vec<TestCellOutput>,
    pub base_cell_deps: Vec<CellDep>,
    pub base_header_deps: Vec<[u8; 32]>,
    pub base_input_masks: Option<Vec<u8>>,
    pub base_output_masks: Option<Vec<u8>>,
    pub base_cell_dep_masks: Option<Vec<u8>>,
    pub base_header_dep_masks: Option<Vec<u8>>,
    pub append_permissions: AppendPermissionsSpec,
    pub append_segments: Vec<AppendSegmentSpec>,
    pub base_seals: Vec<LockSeal>,
}

impl OtxSpec {
    pub fn with_append_permissions(mut self, permissions: AppendPermissionsSpec) -> Self {
        self.append_permissions = permissions;
        self
    }

    pub fn without_append_permissions(self) -> Self {
        self.with_append_permissions(AppendPermissionsSpec::none())
    }
}

#[derive(Clone, Debug, Default)]
pub struct AppendSegmentSpec {
    pub flags: u8,
    pub inputs: Vec<ResolvedInputFacts>,
    pub outputs: Vec<TestCellOutput>,
    pub cell_deps: Vec<CellDep>,
    pub header_deps: Vec<[u8; 32]>,
    pub seals: Vec<LockSeal>,
}

impl AppendSegmentSpec {
    pub fn with_inputs(mut self, inputs: Vec<ResolvedInputFacts>) -> Self {
        self.inputs = inputs;
        self
    }

    pub fn with_outputs(mut self, outputs: Vec<TestCellOutput>) -> Self {
        self.outputs = outputs;
        self
    }

    pub fn with_cell_deps(mut self, cell_deps: Vec<CellDep>) -> Self {
        self.cell_deps = cell_deps;
        self
    }

    pub fn with_header_deps(mut self, header_deps: Vec<[u8; 32]>) -> Self {
        self.header_deps = header_deps;
        self
    }

    pub fn with_seals(mut self, seals: Vec<LockSeal>) -> Self {
        self.seals = seals;
        self
    }
}

pub fn append_segment_spec(flags: u8) -> AppendSegmentSpec {
    AppendSegmentSpec {
        flags,
        ..Default::default()
    }
}

#[derive(Clone, Debug)]
pub struct AppendSegmentRangeFacts {
    pub segment_index: usize,
    pub flags: u8,
    pub inputs: Range<usize>,
    pub outputs: Range<usize>,
    pub cell_deps: Range<usize>,
    pub header_deps: Range<usize>,
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
    pub append_segments: Vec<AppendSegmentRangeFacts>,
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
    pub(crate) otx_witness_start: usize,
}

#[derive(Clone, Debug, Default)]
pub struct TxShape {
    prefix_inputs: Vec<(InputHandle, ResolvedInputFacts)>,
    prefix_cell_deps: Vec<(CellDepHandle, CellDep)>,
    otxs: Vec<TrackedOtx>,
    remainder_outputs: Vec<(OutputHandle, TestCellOutput)>,
    tx_level_message: Option<CobuildMessage>,
    next_input: usize,
    next_output: usize,
    next_cell_dep: usize,
    next_header_dep: usize,
}

#[derive(Clone, Debug)]
struct TrackedOtx {
    handle: OtxHandle,
    segment: OtxSpec,
    base_input_handles: Vec<InputHandle>,
    append_input_handles: Vec<Vec<InputHandle>>,
    base_output_handles: Vec<OutputHandle>,
    append_output_handles: Vec<Vec<OutputHandle>>,
    base_cell_dep_handles: Vec<CellDepHandle>,
    append_cell_dep_handles: Vec<Vec<CellDepHandle>>,
    base_header_dep_handles: Vec<HeaderDepHandle>,
    append_header_dep_handles: Vec<Vec<HeaderDepHandle>>,
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

    pub fn push_otx(&mut self, segment: OtxSpec) -> OtxHandle {
        assert!(
            !segment.base_inputs.is_empty(),
            "OTX segment requires non-zero base inputs"
        );

        let handle = OtxHandle::from_raw(self.otxs.len());
        let base_input_handles = self.input_handles(segment.base_inputs.len());
        let base_output_handles = self.output_handles(segment.base_outputs.len());
        let base_cell_dep_handles = self.cell_dep_handles(segment.base_cell_deps.len());
        let base_header_dep_handles = self.header_dep_handles(segment.base_header_deps.len());
        let append_input_handles = segment
            .append_segments
            .iter()
            .map(|append| self.input_handles(append.inputs.len()))
            .collect();
        let append_output_handles = segment
            .append_segments
            .iter()
            .map(|append| self.output_handles(append.outputs.len()))
            .collect();
        let append_cell_dep_handles = segment
            .append_segments
            .iter()
            .map(|append| self.cell_dep_handles(append.cell_deps.len()))
            .collect();
        let append_header_dep_handles = segment
            .append_segments
            .iter()
            .map(|append| self.header_dep_handles(append.header_deps.len()))
            .collect();

        self.otxs.push(TrackedOtx {
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
        self.otx_append_segment_output(otx, 0, local_index)
    }

    pub fn otx_append_segment_output(
        &self,
        otx: OtxHandle,
        segment_index: usize,
        local_index: usize,
    ) -> OutputHandle {
        self.otx(otx).append_output_handles[segment_index][local_index]
    }

    pub fn otx_base_output(&self, otx: OtxHandle, local_index: usize) -> OutputHandle {
        self.otx(otx).base_output_handles[local_index]
    }

    pub fn otx_base_input(&self, otx: OtxHandle, local_index: usize) -> InputHandle {
        self.otx(otx).base_input_handles[local_index]
    }

    pub fn otx_append_input(&self, otx: OtxHandle, local_index: usize) -> InputHandle {
        self.otx_append_segment_input(otx, 0, local_index)
    }

    pub fn otx_append_segment_input(
        &self,
        otx: OtxHandle,
        segment_index: usize,
        local_index: usize,
    ) -> InputHandle {
        self.otx(otx).append_input_handles[segment_index][local_index]
    }

    pub fn otx_base_cell_dep(&self, otx: OtxHandle, local_index: usize) -> CellDepHandle {
        self.otx(otx).base_cell_dep_handles[local_index]
    }

    pub fn otx_append_cell_dep(&self, otx: OtxHandle, local_index: usize) -> CellDepHandle {
        self.otx_append_segment_cell_dep(otx, 0, local_index)
    }

    pub fn otx_append_segment_cell_dep(
        &self,
        otx: OtxHandle,
        segment_index: usize,
        local_index: usize,
    ) -> CellDepHandle {
        self.otx(otx).append_cell_dep_handles[segment_index][local_index]
    }

    pub fn otx_base_header_dep(&self, otx: OtxHandle, local_index: usize) -> HeaderDepHandle {
        self.otx(otx).base_header_dep_handles[local_index]
    }

    pub fn otx_append_header_dep(&self, otx: OtxHandle, local_index: usize) -> HeaderDepHandle {
        self.otx_append_segment_header_dep(otx, 0, local_index)
    }

    pub fn otx_append_segment_header_dep(
        &self,
        otx: OtxHandle,
        segment_index: usize,
        local_index: usize,
    ) -> HeaderDepHandle {
        self.otx(otx).append_header_dep_handles[segment_index][local_index]
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
                append_segments: otx
                    .segment
                    .append_segments
                    .iter()
                    .enumerate()
                    .map(|(segment_index, append)| AppendSegmentRangeFacts {
                        segment_index,
                        flags: effective_append_segment_flags(
                            append.flags,
                            segment_index,
                            otx.segment.append_segments.len(),
                        ),
                        inputs: 0..0,
                        outputs: 0..0,
                        cell_deps: 0..0,
                        header_deps: 0..0,
                    })
                    .collect(),
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
            for (segment_index, append) in otx.segment.append_segments.iter().enumerate() {
                let segment_start = cell_dep_cursor;
                for (handle, dep) in otx.append_cell_dep_handles[segment_index]
                    .iter()
                    .copied()
                    .zip(append.cell_deps.iter())
                {
                    cell_deps.insert(handle, cell_dep_cursor);
                    builder = builder.cell_dep(dep.clone());
                    cell_dep_cursor += 1;
                }
                otx_ranges[range_index].append_segments[segment_index].cell_deps =
                    segment_start..cell_dep_cursor;
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
            for (segment_index, append) in otx.segment.append_segments.iter().enumerate() {
                let segment_start = header_dep_cursor;
                for (handle, dep) in otx.append_header_dep_handles[segment_index]
                    .iter()
                    .copied()
                    .zip(append.header_deps.iter())
                {
                    header_deps.insert(handle, header_dep_cursor);
                    builder = builder.header_dep(dep.pack());
                    header_dep_cursor += 1;
                }
                otx_ranges[range_index].append_segments[segment_index].header_deps =
                    segment_start..header_dep_cursor;
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
            for (segment_index, append) in otx.segment.append_segments.iter().enumerate() {
                let segment_start = resolved_inputs.len();
                for (handle, input) in otx.append_input_handles[segment_index]
                    .iter()
                    .copied()
                    .zip(append.inputs.iter())
                {
                    inputs.insert(handle, resolved_inputs.len());
                    builder = builder.input(input.input.clone());
                    resolved_inputs.push(input.clone());
                }
                otx_ranges[range_index].append_segments[segment_index].inputs =
                    segment_start..resolved_inputs.len();
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
            for (segment_index, append) in otx.segment.append_segments.iter().enumerate() {
                let segment_start = output_cursor;
                for (handle, output) in otx.append_output_handles[segment_index]
                    .iter()
                    .copied()
                    .zip(append.outputs.iter())
                {
                    outputs.insert(handle, output_cursor);
                    builder = builder
                        .output(output.cell.clone())
                        .output_data(output.data.clone().pack());
                    output_cursor += 1;
                }
                otx_ranges[range_index].append_segments[segment_index].outputs =
                    segment_start..output_cursor;
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
            let append_segments = &otx.segment.append_segments;
            let mut builder_for_otx = OtxBuilder::new()
                .base_input_cells(otx.segment.base_inputs.len() as u32)
                .base_output_cells(otx.segment.base_outputs.len() as u32)
                .base_cell_deps(otx.segment.base_cell_deps.len() as u32)
                .base_header_deps(otx.segment.base_header_deps.len() as u32);
            if append_segments
                .iter()
                .any(|segment| !segment.inputs.is_empty())
            {
                builder_for_otx = builder_for_otx.allow_append_inputs();
            }
            if append_segments
                .iter()
                .any(|segment| !segment.outputs.is_empty())
            {
                builder_for_otx = builder_for_otx.allow_append_outputs();
            }
            if append_segments
                .iter()
                .any(|segment| !segment.cell_deps.is_empty())
            {
                builder_for_otx = builder_for_otx.allow_append_cell_deps();
            }
            if append_segments
                .iter()
                .any(|segment| !segment.header_deps.is_empty())
            {
                builder_for_otx = builder_for_otx.allow_append_header_deps();
            }
            if let AppendPermissionsSpec::Explicit(permissions) = otx.segment.append_permissions {
                builder_for_otx = builder_for_otx.append_permissions(permissions);
            }
            if let Some(masks) = &otx.segment.base_input_masks {
                builder_for_otx = builder_for_otx.base_input_mask_bytes(masks.clone());
            }
            if let Some(masks) = &otx.segment.base_output_masks {
                builder_for_otx = builder_for_otx.base_output_mask_bytes(masks.clone());
            }
            if let Some(masks) = &otx.segment.base_cell_dep_masks {
                builder_for_otx = builder_for_otx.base_cell_dep_mask_bytes(masks.clone());
            }
            if let Some(masks) = &otx.segment.base_header_dep_masks {
                builder_for_otx = builder_for_otx.base_header_dep_mask_bytes(masks.clone());
            }
            if let Some(message) = &otx.segment.message {
                builder_for_otx = builder_for_otx.message(message.clone());
            }
            for append in append_segments {
                builder_for_otx = builder_for_otx.append_segment(
                    append.flags,
                    append.inputs.len() as u32,
                    append.outputs.len() as u32,
                    append.cell_deps.len() as u32,
                    append.header_deps.len() as u32,
                    append.seals.clone(),
                );
            }
            builder_for_otx = builder_for_otx.base_seals(otx.segment.base_seals.clone());

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

    fn otx(&self, otx: OtxHandle) -> &TrackedOtx {
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

fn total_otx_inputs(otxs: &[TrackedOtx]) -> usize {
    otxs.iter()
        .map(|otx| {
            otx.segment.base_inputs.len()
                + otx
                    .segment
                    .append_segments
                    .iter()
                    .map(|segment| segment.inputs.len())
                    .sum::<usize>()
        })
        .sum()
}

fn effective_append_segment_flags(flags: u8, segment_index: usize, segment_count: usize) -> u8 {
    if segment_index + 1 < segment_count {
        flags | 0x01
    } else {
        flags
    }
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

    pub fn otx_append_segment_output(
        &self,
        otx: OtxHandle,
        segment_index: usize,
        local_index: usize,
    ) -> OutputHandle {
        let range = &self.otx_range(otx).append_segments[segment_index].outputs;
        self.outputs
            .handle_at_tx_index(range.start + local_index)
            .expect("append segment output handle")
    }

    pub fn otx_append_segment_input(
        &self,
        otx: OtxHandle,
        segment_index: usize,
        local_index: usize,
    ) -> InputHandle {
        let range = &self.otx_range(otx).append_segments[segment_index].inputs;
        self.inputs
            .handle_at_tx_index(range.start + local_index)
            .expect("append segment input handle")
    }

    pub fn otx_append_segment_cell_dep(
        &self,
        otx: OtxHandle,
        segment_index: usize,
        local_index: usize,
    ) -> CellDepHandle {
        let range = &self.otx_range(otx).append_segments[segment_index].cell_deps;
        self.cell_deps
            .handle_at_tx_index(range.start + local_index)
            .expect("append segment cell dep handle")
    }

    pub fn otx_append_segment_header_dep(
        &self,
        otx: OtxHandle,
        segment_index: usize,
        local_index: usize,
    ) -> HeaderDepHandle {
        let range = &self.otx_range(otx).append_segments[segment_index].header_deps;
        self.header_deps
            .handle_at_tx_index(range.start + local_index)
            .expect("append segment header dep handle")
    }

    fn otx_range(&self, otx: OtxHandle) -> &OtxRangeFacts {
        self.otx_ranges
            .iter()
            .find(|facts| facts.otx == otx)
            .expect("unknown OTX handle")
    }
}

use std::ops::Range;

use ckb_testtool::ckb_types::{bytes::Bytes, prelude::*};
use cobuild_types::entity::witness::WitnessLayout;

use crate::framework::{
    cells::{ResolvedInputFacts, TestCellOutput},
    cobuild::{OtxBuilder, OtxStartSpec},
};

use super::{BuiltTxShape, InputHandle, OtxHandle, OtxRangeFacts, OutputHandle, WitnessHandle};

#[derive(Clone, Debug)]
pub enum ProtocolMutation {
    DuplicateSighashAll,
    NonContiguousOtxWitness,
    OtxBeforeOtxStart,
    OtxStartRaw(OtxStartSpec),
    OtxRawPermission {
        otx: OtxHandle,
        permissions: u8,
    },
    OtxRawBaseInputMasks {
        otx: OtxHandle,
        masks: Vec<u8>,
    },
    SealScopeRaw {
        otx: OtxHandle,
        script_hash: [u8; 32],
        scope: u8,
    },
}

#[derive(Clone, Debug)]
pub enum TxShapeMutation {
    ReplaceInput {
        input: InputHandle,
        replacement: ResolvedInputFacts,
    },
    ReplaceOutput {
        output: OutputHandle,
        replacement: TestCellOutput,
    },
    ReplaceWitness {
        witness: WitnessHandle,
        replacement: Bytes,
    },
    AppendRemainderOutput {
        output: TestCellOutput,
    },
    MoveOutputToRemainder {
        output: OutputHandle,
    },
}

impl BuiltTxShape {
    pub fn apply_protocol_mutation(&mut self, mutation: ProtocolMutation) {
        match mutation {
            ProtocolMutation::OtxStartRaw(spec) => {
                self.replace_witness_bytes(WitnessHandle::from_raw(0), spec.encode());
            }
            ProtocolMutation::OtxRawPermission { otx, permissions } => {
                let builder = self
                    .otx_builder_for_ranges(otx)
                    .append_permissions_raw(permissions);
                self.replace_otx_witness(otx, builder);
            }
            ProtocolMutation::OtxRawBaseInputMasks { otx, masks } => {
                let builder = self.otx_builder_for_ranges(otx).base_input_masks_raw(masks);
                self.replace_otx_witness(otx, builder);
            }
            ProtocolMutation::DuplicateSighashAll => {
                panic!("unsupported protocol mutation: DuplicateSighashAll")
            }
            ProtocolMutation::NonContiguousOtxWitness => {
                panic!("unsupported protocol mutation: NonContiguousOtxWitness")
            }
            ProtocolMutation::OtxBeforeOtxStart => {
                panic!("unsupported protocol mutation: OtxBeforeOtxStart")
            }
            ProtocolMutation::SealScopeRaw { .. } => {
                panic!("unsupported protocol mutation: SealScopeRaw")
            }
        }
    }

    pub fn apply_shape_mutation(&mut self, mutation: TxShapeMutation) -> Option<OutputHandle> {
        match mutation {
            TxShapeMutation::ReplaceInput { input, replacement } => {
                self.replace_input(input, replacement);
                None
            }
            TxShapeMutation::ReplaceOutput {
                output,
                replacement,
            } => {
                self.replace_output(output, replacement);
                None
            }
            TxShapeMutation::ReplaceWitness {
                witness,
                replacement,
            } => {
                self.replace_witness_bytes(witness, replacement);
                None
            }
            TxShapeMutation::AppendRemainderOutput { output } => {
                Some(self.append_remainder_output(output))
            }
            TxShapeMutation::MoveOutputToRemainder { output } => {
                self.move_output_to_remainder(output);
                None
            }
        }
    }

    fn replace_input(&mut self, input: InputHandle, replacement: ResolvedInputFacts) {
        let tx_index = self.inputs.tx_index(input);
        let mut inputs: Vec<_> = self.tx.inputs().into_iter().collect();
        let slot = inputs
            .get_mut(tx_index)
            .expect("input handle points outside transaction inputs");
        *slot = replacement.input.clone();
        self.resolved_inputs[tx_index] = replacement;
        self.tx = self.tx.as_advanced_builder().set_inputs(inputs).build();
    }

    fn replace_output(&mut self, output: OutputHandle, replacement: TestCellOutput) {
        let tx_index = self.outputs.tx_index(output);
        let mut outputs: Vec<_> = self.tx.outputs().into_iter().collect();
        let mut outputs_data: Vec<_> = self.tx.outputs_data().into_iter().collect();
        let output_slot = outputs
            .get_mut(tx_index)
            .expect("output handle points outside transaction outputs");
        let data_slot = outputs_data
            .get_mut(tx_index)
            .expect("output handle points outside transaction output data");
        *output_slot = replacement.cell;
        *data_slot = replacement.data.pack();
        self.tx = self
            .tx
            .as_advanced_builder()
            .set_outputs(outputs)
            .set_outputs_data(outputs_data)
            .build();
    }

    fn replace_witness_bytes(&mut self, witness: WitnessHandle, replacement: Bytes) {
        let tx_index = self.witnesses.tx_index(witness);
        let mut witnesses: Vec<_> = self.tx.witnesses().into_iter().collect();
        let witness_slot = witnesses
            .get_mut(tx_index)
            .expect("witness handle points outside transaction witnesses");
        *witness_slot = replacement.pack();
        self.tx = self
            .tx
            .as_advanced_builder()
            .set_witnesses(witnesses)
            .build();
    }

    fn append_remainder_output(&mut self, output: TestCellOutput) -> OutputHandle {
        let handle = self.next_output_handle();
        let mut outputs: Vec<_> = self.tx.outputs().into_iter().collect();
        let mut outputs_data: Vec<_> = self.tx.outputs_data().into_iter().collect();
        let tx_index = outputs.len();
        outputs.push(output.cell);
        outputs_data.push(output.data.pack());
        self.outputs.set_tx_index(handle, tx_index);
        self.tx = self
            .tx
            .as_advanced_builder()
            .set_outputs(outputs)
            .set_outputs_data(outputs_data)
            .build();
        handle
    }

    fn move_output_to_remainder(&mut self, output: OutputHandle) {
        let old_index = self.outputs.tx_index(output);
        let mut outputs: Vec<_> = self.tx.outputs().into_iter().collect();
        let mut outputs_data: Vec<_> = self.tx.outputs_data().into_iter().collect();
        assert_eq!(
            outputs.len(),
            outputs_data.len(),
            "transaction outputs and output data must have matching lengths"
        );
        assert!(
            old_index < outputs.len(),
            "output handle points outside transaction outputs"
        );

        let moved_output = outputs.remove(old_index);
        let moved_data = outputs_data.remove(old_index);
        let new_index = outputs.len();
        outputs.push(moved_output);
        outputs_data.push(moved_data);

        self.outputs.remap_tx_indexes(|tx_index| {
            if tx_index == old_index {
                new_index
            } else if tx_index > old_index {
                tx_index - 1
            } else {
                tx_index
            }
        });
        for facts in &mut self.otx_ranges {
            move_index_out_of_otx_range(&mut facts.base_outputs, old_index);
            move_index_out_of_otx_range(&mut facts.append_outputs, old_index);
        }
        self.tx = self
            .tx
            .as_advanced_builder()
            .set_outputs(outputs)
            .set_outputs_data(outputs_data)
            .build();
    }

    fn next_output_handle(&self) -> OutputHandle {
        let next = self
            .outputs
            .handles()
            .map(|handle| handle.0)
            .max()
            .map(|max| max + 1)
            .unwrap_or(0);
        OutputHandle::from_raw(next)
    }

    fn replace_otx_witness(&mut self, otx: OtxHandle, builder: OtxBuilder) {
        let witness = WitnessLayout::from(builder.build());
        self.replace_witness_bytes(
            WitnessHandle::from_raw(otx.0 + 1),
            Bytes::copy_from_slice(witness.as_slice()),
        );
    }

    fn otx_builder_for_ranges(&self, otx: OtxHandle) -> OtxBuilder {
        let facts = self.otx_range_facts(otx);
        let mut builder = OtxBuilder::new()
            .base_input_cells(range_len(&facts.base_inputs) as u32)
            .base_output_cells(range_len(&facts.base_outputs) as u32)
            .base_cell_deps(range_len(&facts.base_cell_deps) as u32)
            .base_header_deps(range_len(&facts.base_header_deps) as u32)
            .append_input_cells(range_len(&facts.append_inputs) as u32)
            .append_output_cells(range_len(&facts.append_outputs) as u32)
            .append_cell_deps(range_len(&facts.append_cell_deps) as u32)
            .append_header_deps(range_len(&facts.append_header_deps) as u32);
        if !facts.append_inputs.is_empty() {
            builder = builder.allow_append_inputs();
        }
        if !facts.append_outputs.is_empty() {
            builder = builder.allow_append_outputs();
        }
        if !facts.append_cell_deps.is_empty() {
            builder = builder.allow_append_cell_deps();
        }
        if !facts.append_header_deps.is_empty() {
            builder = builder.allow_append_header_deps();
        }
        builder
    }

    fn otx_range_facts(&self, otx: OtxHandle) -> &OtxRangeFacts {
        self.otx_ranges
            .iter()
            .find(|facts| facts.otx == otx)
            .expect("unknown OTX handle")
    }
}

fn move_index_out_of_otx_range(range: &mut Range<usize>, old_index: usize) {
    if old_index < range.start {
        range.start -= 1;
        range.end -= 1;
    } else if range.contains(&old_index) {
        range.end -= 1;
    }
}

fn range_len(range: &Range<usize>) -> usize {
    range.end.saturating_sub(range.start)
}

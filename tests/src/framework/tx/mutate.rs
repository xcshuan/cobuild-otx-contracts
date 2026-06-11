use std::ops::Range;

use ckb_testtool::ckb_types::{bytes::Bytes, prelude::*};
use cobuild_types::entity::{
    core::{Otx, SealPair, SealPairVec, SighashAll, SighashAllOnly},
    witness::{WitnessLayout, WitnessLayoutUnion},
};

use crate::framework::{
    cells::{ResolvedInputFacts, TestCellOutput},
    cobuild::{OtxStartSpec, empty_message},
};

use super::{BuiltTxShape, InputHandle, OtxHandle, OutputHandle, WitnessHandle};

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
                let updated = self
                    .current_otx_witness(otx)
                    .as_builder()
                    .append_permissions(permissions)
                    .build();
                self.replace_otx_witness(otx, updated);
            }
            ProtocolMutation::OtxRawBaseInputMasks { otx, masks } => {
                let updated = self
                    .current_otx_witness(otx)
                    .as_builder()
                    .base_input_masks(masks)
                    .build();
                self.replace_otx_witness(otx, updated);
            }
            ProtocolMutation::DuplicateSighashAll => {
                let witness = WitnessLayout::from(
                    SighashAll::new_builder()
                        .message(empty_message())
                        .seal(Vec::<u8>::new())
                        .build(),
                );
                let bytes = Bytes::copy_from_slice(witness.as_slice());
                self.insert_witness_bytes(0, bytes.clone());
                self.insert_witness_bytes(1, bytes);
            }
            ProtocolMutation::NonContiguousOtxWitness => {
                let witness = WitnessLayout::from(
                    SighashAllOnly::new_builder().seal(Vec::<u8>::new()).build(),
                );
                self.insert_witness_bytes(1, Bytes::copy_from_slice(witness.as_slice()));
            }
            ProtocolMutation::OtxBeforeOtxStart => {
                self.swap_witnesses(0, 1);
            }
            ProtocolMutation::SealScopeRaw {
                otx,
                script_hash,
                scope,
            } => {
                let updated = self
                    .current_otx_witness(otx)
                    .with_raw_seal_scope(script_hash, scope);
                self.replace_otx_witness(otx, updated);
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

    fn insert_witness_bytes(&mut self, tx_index: usize, witness: Bytes) {
        let mut witnesses: Vec<_> = self.tx.witnesses().into_iter().collect();
        assert!(
            tx_index <= witnesses.len(),
            "witness insertion index points outside transaction witnesses"
        );
        witnesses.insert(tx_index, witness.pack());
        self.witnesses.remap_tx_indexes(|current| {
            if current >= tx_index {
                current + 1
            } else {
                current
            }
        });
        self.tx = self
            .tx
            .as_advanced_builder()
            .set_witnesses(witnesses)
            .build();
    }

    fn swap_witnesses(&mut self, left: usize, right: usize) {
        let mut witnesses: Vec<_> = self.tx.witnesses().into_iter().collect();
        assert!(
            left < witnesses.len() && right < witnesses.len(),
            "witness swap index points outside transaction witnesses"
        );
        witnesses.swap(left, right);
        self.witnesses.remap_tx_indexes(|current| {
            if current == left {
                right
            } else if current == right {
                left
            } else {
                current
            }
        });
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
        let witness_updates = self.otx_witness_updates_for_moved_output(old_index);
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
        for (otx, updated) in witness_updates {
            self.replace_otx_witness(otx, updated);
        }
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

    fn replace_otx_witness(&mut self, otx: OtxHandle, otx_entity: Otx) {
        let witness = WitnessLayout::from(otx_entity);
        self.replace_witness_bytes(
            WitnessHandle::from_raw(otx.0 + 1),
            Bytes::copy_from_slice(witness.as_slice()),
        );
    }

    fn current_otx_witness(&self, otx: OtxHandle) -> Otx {
        let witness_index = self.witnesses.tx_index(WitnessHandle::from_raw(otx.0 + 1));
        let witness = self
            .tx
            .witnesses()
            .into_iter()
            .nth(witness_index)
            .expect("OTX witness handle points outside transaction witnesses");
        match WitnessLayout::from_slice(witness.raw_data().as_ref())
            .expect("parse cobuild witness layout")
            .to_enum()
        {
            WitnessLayoutUnion::Otx(otx) => otx,
            other => panic!("expected OTX witness, got {}", other.item_name()),
        }
    }

    fn otx_witness_updates_for_moved_output(&self, old_index: usize) -> Vec<(OtxHandle, Otx)> {
        let mut updates = Vec::new();
        for facts in &self.otx_ranges {
            let in_base = facts.base_outputs.contains(&old_index);
            let in_append = facts.append_outputs.contains(&old_index);
            assert!(
                !(in_base && in_append),
                "output index belongs to both base and append OTX output ranges"
            );
            if in_base {
                updates.push((
                    facts.otx,
                    self.current_otx_witness(facts.otx)
                        .decrement_base_output_count(old_index - facts.base_outputs.start),
                ));
            }
            if in_append {
                updates.push((
                    facts.otx,
                    self.current_otx_witness(facts.otx)
                        .decrement_append_output_count(),
                ));
            }
        }
        assert!(
            updates.len() <= 1,
            "output index belongs to multiple OTX output ranges"
        );
        updates
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

trait OtxOutputCountMutation {
    fn decrement_base_output_count(self, removed_local_index: usize) -> Self;
    fn decrement_append_output_count(self) -> Self;
}

impl OtxOutputCountMutation for Otx {
    fn decrement_base_output_count(self, removed_local_index: usize) -> Self {
        let old_count = u32_field(self.base_output_cells(), "base_output_cells");
        assert!(old_count > 0, "cannot decrement zero base_output_cells");
        assert!(
            removed_local_index < old_count as usize,
            "removed base output local index out of range"
        );
        let new_count = old_count - 1;
        let masks = remove_base_output_mask_group(
            self.base_output_masks().raw_data().as_ref(),
            old_count as usize,
            removed_local_index,
        );
        self.as_builder()
            .base_output_cells(new_count.to_le_bytes())
            .base_output_masks(masks)
            .build()
    }

    fn decrement_append_output_count(self) -> Self {
        let old_count = u32_field(self.append_output_cells(), "append_output_cells");
        assert!(old_count > 0, "cannot decrement zero append_output_cells");
        self.as_builder()
            .append_output_cells((old_count - 1).to_le_bytes())
            .build()
    }
}

trait OtxSealMutation {
    fn with_raw_seal_scope(self, script_hash: [u8; 32], scope: u8) -> Self;
}

impl OtxSealMutation for Otx {
    fn with_raw_seal_scope(self, script_hash: [u8; 32], scope: u8) -> Self {
        let mut replaced = false;
        let mut seals: Vec<_> = self
            .seals()
            .into_iter()
            .map(|seal| {
                if seal.script_hash().raw_data().as_ref() == script_hash {
                    replaced = true;
                    seal.as_builder().scope(scope).build()
                } else {
                    seal
                }
            })
            .collect();
        if !replaced {
            seals.push(
                SealPair::new_builder()
                    .script_hash(script_hash)
                    .scope(scope)
                    .seal(Vec::<u8>::new())
                    .build(),
            );
        }

        self.as_builder()
            .seals(SealPairVec::new_builder().extend(seals).build())
            .build()
    }
}

fn u32_field(value: cobuild_types::entity::blockchain::Uint32, field: &str) -> u32 {
    u32::from_le_bytes(value.as_slice().try_into().expect(field))
}

fn remove_base_output_mask_group(
    masks: &[u8],
    old_count: usize,
    removed_local_index: usize,
) -> Vec<u8> {
    let old_bits = old_count * 4;
    let removed_start = removed_local_index * 4;
    let removed_end = removed_start + 4;
    let new_bits = old_bits.saturating_sub(4);
    let mut updated = vec![0u8; new_bits.div_ceil(8)];
    let mut new_bit = 0;

    for old_bit in 0..old_bits {
        if (removed_start..removed_end).contains(&old_bit) {
            continue;
        }
        if mask_bit(masks, old_bit) {
            set_mask_bit(&mut updated, new_bit);
        }
        new_bit += 1;
    }

    clear_padding_bits(&mut updated, new_bits);
    updated
}

fn mask_bit(masks: &[u8], bit: usize) -> bool {
    masks
        .get(bit / 8)
        .map(|byte| byte & (1 << (bit % 8)) != 0)
        .unwrap_or(false)
}

fn set_mask_bit(masks: &mut [u8], bit: usize) {
    masks[bit / 8] |= 1 << (bit % 8);
}

fn clear_padding_bits(masks: &mut [u8], bit_count: usize) {
    let used_bits = bit_count % 8;
    if used_bits == 0 || masks.is_empty() {
        return;
    }
    let keep_mask = (1u8 << used_bits) - 1;
    let last = masks.last_mut().expect("non-empty mask");
    *last &= keep_mask;
}

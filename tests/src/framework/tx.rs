use ckb_testtool::ckb_types::{bytes::Bytes, prelude::*};
use cobuild_types::entity::{core::OtxStart, witness::WitnessLayout};

#[path = "tx/builder.rs"]
pub mod builder;
#[path = "tx/handles.rs"]
pub mod handles;
#[path = "tx/malformed.rs"]
pub mod malformed;
#[path = "tx/mutate.rs"]
pub mod mutate;

pub use builder::{BuiltTxShape, OtxRangeFacts, OtxSegment, TxShape};
pub use handles::{
    CellDepHandle, EntityIndexMap, HeaderDepHandle, InputHandle, OtxHandle, OutputHandle,
    WitnessHandle,
};
pub use mutate::{ProtocolMutation, TxShapeMutation};

pub fn otx_start_witness(
    start_input_cell: u32,
    start_output_cell: u32,
    start_cell_deps: u32,
    start_header_deps: u32,
) -> Bytes {
    let witness = WitnessLayout::from(
        OtxStart::new_builder()
            .start_input_cell(start_input_cell.to_le_bytes())
            .start_output_cell(start_output_cell.to_le_bytes())
            .start_cell_deps(start_cell_deps.to_le_bytes())
            .start_header_deps(start_header_deps.to_le_bytes())
            .build(),
    );
    Bytes::copy_from_slice(witness.as_slice())
}

#[cfg(test)]
mod tests {
    use ckb_testtool::ckb_types::{
        bytes::Bytes,
        packed::{CellDep, CellInput, CellOutput, OutPoint, Script},
        prelude::{Builder, Entity, Pack},
    };
    use cobuild_types::entity::{
        blockchain::Uint32,
        core::OtxStart,
        witness::{WitnessLayout, WitnessLayoutUnion},
    };

    use super::{BuiltTxShape, OtxSegment, TxShape};
    use crate::framework::cells::{ResolvedInputFacts, TestCellOutput, normal_output};

    fn empty_script() -> Script {
        Script::new_builder().build()
    }

    fn resolved_input(tag: u8) -> ResolvedInputFacts {
        let lock = empty_script();
        let output = normal_output(lock, 1_000 + u64::from(tag));

        ResolvedInputFacts {
            input: CellInput::new_builder().build(),
            output,
            data: Bytes::from(vec![tag]),
            lock_hash: [tag; 32],
            type_hash: None,
        }
    }

    fn output(tag: u8) -> TestCellOutput {
        TestCellOutput::new(
            CellOutput::new_builder()
                .capacity(1_000 + u64::from(tag))
                .lock(empty_script())
                .build(),
            vec![tag],
        )
    }

    fn cell_dep(tag: u8) -> CellDep {
        CellDep::new_builder()
            .out_point(OutPoint::new([tag; 32].pack(), 0))
            .build()
    }

    fn molecule_u32(value: Uint32) -> u32 {
        u32::from_le_bytes(value.as_slice().try_into().expect("uint32 bytes"))
    }

    fn otx_start_witness_entity(built: &BuiltTxShape) -> OtxStart {
        let witness = built
            .tx
            .witnesses()
            .into_iter()
            .nth(built.witnesses.tx_index(built.otx_start_witness()))
            .expect("OTX start witness")
            .raw_data();
        match WitnessLayout::from_slice(witness.as_ref())
            .expect("parse witness layout")
            .to_enum()
        {
            WitnessLayoutUnion::OtxStart(start) => start,
            other => panic!("expected OtxStart witness, got {}", other.item_name()),
        }
    }

    #[test]
    fn tx_shape_maps_two_otxs_and_remainder_output_handles() {
        let mut shape = TxShape::new();
        let prefix = shape.push_prefix_input(resolved_input(1));
        let prefix_dep = shape.push_prefix_cell_dep(cell_dep(9));

        let first_otx = shape.push_otx(OtxSegment {
            base_inputs: vec![resolved_input(2)],
            append_inputs: vec![resolved_input(3)],
            base_outputs: vec![output(10)],
            append_outputs: vec![output(11), output(12)],
            base_cell_deps: vec![cell_dep(10)],
            append_cell_deps: vec![cell_dep(11), cell_dep(12)],
            base_header_deps: vec![[10; 32]],
            append_header_deps: vec![[11; 32], [12; 32]],
            ..Default::default()
        });
        let second_otx = shape.push_otx(OtxSegment {
            base_inputs: vec![resolved_input(4)],
            append_inputs: vec![resolved_input(5)],
            base_outputs: vec![output(20)],
            append_outputs: vec![output(21)],
            base_cell_deps: vec![cell_dep(20)],
            append_cell_deps: vec![cell_dep(21)],
            base_header_deps: vec![[20; 32]],
            append_header_deps: vec![[21; 32]],
            ..Default::default()
        });
        let remainder = shape.push_remainder_output(output(30));

        let first_base = shape.otx_base_output(first_otx, 0);
        let second_base = shape.otx_base_output(second_otx, 0);
        let first_base_input = shape.otx_base_input(first_otx, 0);
        let first_append_input = shape.otx_append_input(first_otx, 0);
        let first_append_0 = shape.otx_append_output(first_otx, 0);
        let first_append_1 = shape.otx_append_output(first_otx, 1);
        let second_append = shape.otx_append_output(second_otx, 0);

        let built = shape.build();

        assert_eq!(built.inputs.tx_index(prefix), 0);
        assert_eq!(built.inputs.handle_at_tx_index(0), Some(prefix));
        assert_eq!(built.inputs.tx_index(first_base_input), 1);
        assert_eq!(built.inputs.tx_index(first_append_input), 2);
        assert_eq!(built.cell_deps.tx_index(prefix_dep), 0);
        assert_eq!(built.outputs.tx_index(first_base), 0);
        assert_eq!(built.outputs.tx_index(first_append_0), 1);
        assert_eq!(built.outputs.tx_index(first_append_1), 2);
        assert_eq!(built.outputs.tx_index(second_base), 3);
        assert_eq!(built.outputs.tx_index(second_append), 4);
        assert_eq!(built.outputs.tx_index(remainder), 5);
        assert_eq!(built.outputs.handle_at_tx_index(5), Some(remainder));

        assert_eq!(built.tx.inputs().len(), 5);
        assert_eq!(built.tx.outputs().len(), 6);
        assert_eq!(built.tx.cell_deps().len(), 6);
        assert_eq!(built.tx.header_deps().len(), 5);
        assert_eq!(built.resolved_inputs.len(), 5);
        assert_eq!(built.otx_ranges.len(), 2);
        assert_eq!(built.otx_ranges[0].base_inputs, 1..2);
        assert_eq!(built.otx_ranges[0].append_inputs, 2..3);
        assert_eq!(built.otx_ranges[0].base_outputs, 0..1);
        assert_eq!(built.otx_ranges[0].append_outputs, 1..3);
        assert_eq!(built.otx_ranges[0].base_cell_deps, 1..2);
        assert_eq!(built.otx_ranges[0].append_cell_deps, 2..4);
        assert_eq!(built.otx_ranges[0].base_header_deps, 0..1);
        assert_eq!(built.otx_ranges[0].append_header_deps, 1..3);
        assert_eq!(built.otx_ranges[1].base_inputs, 3..4);
        assert_eq!(built.otx_ranges[1].append_inputs, 4..5);
        assert_eq!(built.otx_ranges[1].base_outputs, 3..4);
        assert_eq!(built.otx_ranges[1].append_outputs, 4..5);
        assert_eq!(built.otx_ranges[1].base_cell_deps, 4..5);
        assert_eq!(built.otx_ranges[1].append_cell_deps, 5..6);
        assert_eq!(built.otx_ranges[1].base_header_deps, 3..4);
        assert_eq!(built.otx_ranges[1].append_header_deps, 4..5);

        let start = otx_start_witness_entity(&built);
        assert_eq!(molecule_u32(start.start_input_cell()), 1);
        assert_eq!(molecule_u32(start.start_cell_deps()), 1);
    }

    #[test]
    #[should_panic(expected = "OTX segment requires non-zero base inputs")]
    fn tx_shape_rejects_zero_base_input_otx_segments() {
        let mut shape = TxShape::new();
        shape.push_otx(OtxSegment::default());
    }
}

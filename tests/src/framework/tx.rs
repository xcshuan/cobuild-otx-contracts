use ckb_testtool::ckb_types::{
    bytes::Bytes,
    core::{TransactionBuilder, TransactionView},
    packed::{CellDep, CellInput},
    prelude::*,
};
use cobuild_types::entity::{
    core::{Message as CobuildMessage, OtxStart, SighashAll},
    witness::WitnessLayout,
};

use super::cells::TestCellOutput;
use super::cobuild::BuiltOtx;

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

#[derive(Clone, Debug, Default)]
pub struct OtxTransactionBuilder {
    cell_deps: Vec<CellDep>,
    base_inputs: Vec<CellInput>,
    append_inputs: Vec<CellInput>,
    base_outputs: Vec<TestCellOutput>,
    append_outputs: Vec<TestCellOutput>,
    remainder_outputs: Vec<TestCellOutput>,
    tx_level_message: Option<CobuildMessage>,
    otxs: Vec<BuiltOtx>,
    allow_no_otx: bool,
}

impl OtxTransactionBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cell_dep(mut self, dep: CellDep) -> Self {
        self.cell_deps.push(dep);
        self
    }

    pub fn cell_deps(mut self, deps: impl IntoIterator<Item = CellDep>) -> Self {
        self.cell_deps.extend(deps);
        self
    }

    pub fn base_input(mut self, input: CellInput) -> Self {
        self.base_inputs.push(input);
        self
    }

    pub fn append_input(mut self, input: CellInput) -> Self {
        self.append_inputs.push(input);
        self
    }

    pub fn base_output(mut self, output: TestCellOutput) -> Self {
        self.base_outputs.push(output);
        self
    }

    pub fn append_output(mut self, output: TestCellOutput) -> Self {
        self.append_outputs.push(output);
        self
    }

    pub fn remainder_output(mut self, output: TestCellOutput) -> Self {
        self.remainder_outputs.push(output);
        self
    }

    pub fn otx(mut self, otx: BuiltOtx) -> Self {
        self.otxs.push(otx);
        self
    }

    pub fn tx_level_message(mut self, message: CobuildMessage) -> Self {
        self.tx_level_message = Some(message);
        self
    }

    pub fn allow_no_otx(mut self) -> Self {
        self.allow_no_otx = true;
        self
    }

    pub fn build(self) -> TransactionView {
        assert!(
            self.allow_no_otx || !self.otxs.is_empty(),
            "OTX transaction requires one Otx unless allow_no_otx is set"
        );
        assert!(
            !self.base_inputs.is_empty(),
            "transaction requires non-zero base inputs"
        );
        if !self.otxs.is_empty() {
            assert!(
                self.otxs.iter().all(|otx| otx.base_input_cells > 0),
                "each OTX requires non-zero base inputs"
            );
        }

        let start_input_cell = 0u32;
        let start_output_cell = 0u32;
        let start_cell_deps = self.cell_deps.len() as u32;
        let start_header_deps = 0u32;

        let total_base_inputs: u32 = self.otxs.iter().map(|otx| otx.base_input_cells).sum();
        let total_base_outputs: u32 = self.otxs.iter().map(|otx| otx.base_output_cells).sum();
        let total_base_cell_deps: u32 = self.otxs.iter().map(|otx| otx.base_cell_deps).sum();
        let total_base_header_deps: u32 = self.otxs.iter().map(|otx| otx.base_header_deps).sum();
        let total_append_inputs: u32 = self.otxs.iter().map(|otx| otx.append_input_cells).sum();
        let total_append_outputs: u32 = self.otxs.iter().map(|otx| otx.append_output_cells).sum();
        let total_append_cell_deps: u32 = self.otxs.iter().map(|otx| otx.append_cell_deps).sum();
        let total_append_header_deps: u32 =
            self.otxs.iter().map(|otx| otx.append_header_deps).sum();
        assert!(
            total_base_inputs as usize <= self.base_inputs.len(),
            "OTX base input range exceeds transaction inputs"
        );
        assert!(
            total_base_outputs as usize <= self.base_outputs.len(),
            "OTX base output range exceeds transaction outputs"
        );
        assert!(
            total_base_cell_deps as usize <= self.cell_deps.len(),
            "OTX base cell dep range exceeds transaction cell deps"
        );
        assert!(
            total_base_header_deps == 0,
            "OTX base header dep range exceeds transaction header deps"
        );
        assert!(
            total_append_inputs as usize <= self.append_inputs.len(),
            "OTX append input range exceeds transaction inputs"
        );
        assert!(
            total_append_outputs as usize <= self.append_outputs.len(),
            "OTX append output range exceeds transaction outputs"
        );
        assert!(
            total_append_cell_deps == 0,
            "OTX append cell dep range exceeds transaction cell deps"
        );
        assert!(
            total_append_header_deps == 0,
            "OTX append header dep range exceeds transaction header deps"
        );

        let mut builder = TransactionBuilder::default();
        for dep in self.cell_deps {
            builder = builder.cell_dep(dep);
        }
        for input in self.base_inputs {
            builder = builder.input(input);
        }
        for input in self.append_inputs {
            builder = builder.input(input);
        }
        for output in self.base_outputs {
            builder = builder.output(output.cell).output_data(output.data.pack());
        }
        for output in self.append_outputs {
            builder = builder.output(output.cell).output_data(output.data.pack());
        }
        for output in self.remainder_outputs {
            builder = builder.output(output.cell).output_data(output.data.pack());
        }

        if let Some(message) = self.tx_level_message {
            let witness = WitnessLayout::from(
                SighashAll::new_builder()
                    .seal(Vec::<u8>::new())
                    .message(message)
                    .build(),
            );
            builder = builder.witness(Bytes::copy_from_slice(witness.as_slice()).pack());
        }
        if !self.otxs.is_empty() {
            builder = builder.witness(
                otx_start_witness(
                    start_input_cell,
                    start_output_cell,
                    start_cell_deps,
                    start_header_deps,
                )
                .pack(),
            );
        }
        for otx in self.otxs {
            let witness = WitnessLayout::from(otx.otx);
            builder = builder.witness(Bytes::copy_from_slice(witness.as_slice()).pack());
        }

        builder.build()
    }
}

#[cfg(test)]
mod tests {
    use ckb_testtool::ckb_types::{
        bytes::Bytes,
        packed::{CellDep, CellInput, CellOutput, OutPoint, Script},
        prelude::{Builder, Entity, Pack},
    };

    use super::{OtxSegment, TxShape};
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

    #[test]
    fn tx_shape_maps_two_otxs_and_remainder_output_handles() {
        let mut shape = TxShape::new();
        let prefix = shape.push_prefix_input(resolved_input(1));

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
        let first_append_0 = shape.otx_append_output(first_otx, 0);
        let first_append_1 = shape.otx_append_output(first_otx, 1);
        let second_append = shape.otx_append_output(second_otx, 0);

        let built = shape.build();

        assert_eq!(built.inputs.tx_index(prefix), 0);
        assert_eq!(built.inputs.handle_at_tx_index(0), Some(prefix));
        assert_eq!(built.outputs.tx_index(first_base), 0);
        assert_eq!(built.outputs.tx_index(first_append_0), 1);
        assert_eq!(built.outputs.tx_index(first_append_1), 2);
        assert_eq!(built.outputs.tx_index(second_base), 3);
        assert_eq!(built.outputs.tx_index(second_append), 4);
        assert_eq!(built.outputs.tx_index(remainder), 5);
        assert_eq!(built.outputs.handle_at_tx_index(5), Some(remainder));

        assert_eq!(built.tx.inputs().len(), 5);
        assert_eq!(built.tx.outputs().len(), 6);
        assert_eq!(built.tx.cell_deps().len(), 5);
        assert_eq!(built.tx.header_deps().len(), 5);
        assert_eq!(built.resolved_inputs.len(), 5);
        assert_eq!(built.otx_ranges.len(), 2);
        assert_eq!(built.otx_ranges[0].base_inputs, 1..2);
        assert_eq!(built.otx_ranges[0].append_inputs, 2..3);
        assert_eq!(built.otx_ranges[0].base_outputs, 0..1);
        assert_eq!(built.otx_ranges[0].append_outputs, 1..3);
        assert_eq!(built.otx_ranges[0].base_cell_deps, 0..1);
        assert_eq!(built.otx_ranges[0].append_cell_deps, 1..3);
        assert_eq!(built.otx_ranges[0].base_header_deps, 0..1);
        assert_eq!(built.otx_ranges[0].append_header_deps, 1..3);
        assert_eq!(built.otx_ranges[1].base_inputs, 3..4);
        assert_eq!(built.otx_ranges[1].append_inputs, 4..5);
        assert_eq!(built.otx_ranges[1].base_outputs, 3..4);
        assert_eq!(built.otx_ranges[1].append_outputs, 4..5);
        assert_eq!(built.otx_ranges[1].base_cell_deps, 3..4);
        assert_eq!(built.otx_ranges[1].append_cell_deps, 4..5);
        assert_eq!(built.otx_ranges[1].base_header_deps, 3..4);
        assert_eq!(built.otx_ranges[1].append_header_deps, 4..5);
    }

    #[test]
    #[should_panic(expected = "OTX segment requires non-zero base inputs")]
    fn tx_shape_rejects_zero_base_input_otx_segments() {
        let mut shape = TxShape::new();
        shape.push_otx(OtxSegment::default());
    }
}

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
        if !self.allow_no_otx {
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

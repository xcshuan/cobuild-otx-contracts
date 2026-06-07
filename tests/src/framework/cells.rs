use ckb_testtool::{
    ckb_types::{
        bytes::Bytes,
        packed::{CellInput, CellOutput, OutPoint, Script},
        prelude::*,
    },
    context::Context,
};

#[derive(Clone, Debug)]
pub struct TestCellOutput {
    pub cell: CellOutput,
    pub data: Bytes,
}

#[derive(Clone, Debug)]
pub struct TestResolvedInput {
    pub raw_input: Vec<u8>,
    pub resolved_output: Vec<u8>,
    pub data: Vec<u8>,
}

impl TestCellOutput {
    pub fn new(cell: CellOutput, data: impl Into<Bytes>) -> Self {
        Self {
            cell,
            data: data.into(),
        }
    }
}

pub fn normal_output(lock: Script, capacity: u64) -> CellOutput {
    CellOutput::new_builder()
        .capacity(capacity)
        .lock(lock)
        .build()
}

pub fn typed_output(lock: Script, type_script: Script, capacity: u64) -> CellOutput {
    CellOutput::new_builder()
        .capacity(capacity)
        .lock(lock)
        .type_(Some(type_script).pack())
        .build()
}

pub fn live_input(context: &mut Context, output: CellOutput, data: impl Into<Bytes>) -> CellInput {
    let out_point: OutPoint = context.create_cell(output, data.into());
    CellInput::new_builder().previous_output(out_point).build()
}

pub fn live_resolved_input(
    context: &mut Context,
    output: CellOutput,
    data: impl Into<Bytes>,
) -> (CellInput, TestResolvedInput) {
    let data = data.into();
    let previous_output: OutPoint = context.create_cell(output.clone(), data.clone());
    let input = CellInput::new_builder()
        .previous_output(previous_output)
        .build();
    let resolved = TestResolvedInput {
        raw_input: input.as_slice().to_vec(),
        resolved_output: output.as_slice().to_vec(),
        data: data.to_vec(),
    };
    (input, resolved)
}

pub fn live_resolved_normal_input(
    context: &mut Context,
    lock: Script,
    capacity: u64,
    data: impl Into<Bytes>,
) -> (CellInput, TestResolvedInput) {
    live_resolved_input(context, normal_output(lock, capacity), data)
}

pub fn live_resolved_typed_input(
    context: &mut Context,
    lock: Script,
    type_script: Script,
    capacity: u64,
    data: impl Into<Bytes>,
) -> (CellInput, TestResolvedInput) {
    live_resolved_input(context, typed_output(lock, type_script, capacity), data)
}

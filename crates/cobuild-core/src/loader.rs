use alloc::vec::Vec;
use core::convert::TryInto;

use cobuild_types::lazy_reader::blockchain::{Script, Transaction};

use crate::{
    context::{CobuildContext, PreparedContext, TxScriptHashes},
    error::CoreError,
    layout::LayoutTx,
    reader::{cursor_bytes, cursor_from_slice},
    source::InMemorySource,
};

pub struct TransactionInfo {
    pub transaction: Vec<u8>,
    pub witnesses: Vec<Vec<u8>>,
    pub raw_inputs: Vec<Vec<u8>>,
    pub raw_outputs: Vec<Vec<u8>>,
    pub raw_outputs_data: Vec<Vec<u8>>,
    pub raw_cell_deps: Vec<Vec<u8>>,
    pub raw_header_deps: Vec<[u8; 32]>,
    pub input_count: usize,
    pub output_count: usize,
    pub cell_dep_count: usize,
    pub header_dep_count: usize,
}

pub struct PreparedContextInput {
    pub transaction: Vec<u8>,
    pub script: Vec<u8>,
    pub witnesses: Vec<Vec<u8>>,
    pub input_count: usize,
    pub output_count: usize,
    pub cell_dep_count: usize,
    pub header_dep_count: usize,
    pub input_locks: Vec<[u8; 32]>,
    pub input_types: Vec<Option<[u8; 32]>>,
    pub output_types: Vec<Option<[u8; 32]>>,
    pub tx_hash: [u8; 32],
    pub resolved_outputs: Vec<Vec<u8>>,
    pub resolved_data: Vec<Vec<u8>>,
    pub raw_inputs: Vec<Vec<u8>>,
    pub raw_outputs: Vec<Vec<u8>>,
    pub raw_outputs_data: Vec<Vec<u8>>,
    pub raw_cell_deps: Vec<Vec<u8>>,
    pub raw_header_deps: Vec<[u8; 32]>,
}

pub fn prepare_context(input: PreparedContextInput) -> Result<PreparedContext, CoreError> {
    let context = CobuildContext::new(
        LayoutTx {
            witnesses: input.witnesses.clone(),
            input_count: input.input_count,
            output_count: input.output_count,
            cell_dep_count: input.cell_dep_count,
            header_dep_count: input.header_dep_count,
        },
        TxScriptHashes {
            input_locks: input.input_locks.clone(),
            input_types: input.input_types.clone(),
            output_types: input.output_types.clone(),
        },
    )?;
    let signing_source = InMemorySource {
        transaction: input.transaction,
        script: input.script,
        tx_hash: input.tx_hash,
        input_locks: input.input_locks,
        input_types: input.input_types,
        output_types: input.output_types,
        resolved_outputs: input.resolved_outputs,
        resolved_data: input.resolved_data,
        raw_inputs: input.raw_inputs,
        raw_outputs: input.raw_outputs,
        raw_outputs_data: input.raw_outputs_data,
        raw_cell_deps: input.raw_cell_deps,
        raw_header_deps: input.raw_header_deps,
        witnesses: input.witnesses,
    };

    Ok(PreparedContext::new(context, signing_source))
}

pub fn parse_transaction_info(data: &[u8]) -> Result<TransactionInfo, CoreError> {
    let tx = Transaction::from(cursor_from_slice(data));
    tx.verify(false).map_err(|_| CoreError::InvalidOtxLayout)?;
    let raw = tx.raw().map_err(|_| CoreError::MalformedCobuild)?;
    let witnesses_reader = tx.witnesses().map_err(|_| CoreError::MalformedCobuild)?;
    let witness_count = witnesses_reader
        .len()
        .map_err(|_| CoreError::MalformedCobuild)?;
    let mut witnesses = Vec::with_capacity(witness_count);
    for index in 0..witness_count {
        let witness = witnesses_reader
            .get(index)
            .and_then(TryInto::try_into)
            .map_err(|_| CoreError::MalformedCobuild)?;
        witnesses.push(witness);
    }

    let inputs = raw.inputs().map_err(|_| CoreError::MalformedCobuild)?;
    let outputs = raw.outputs().map_err(|_| CoreError::MalformedCobuild)?;
    let outputs_data = raw
        .outputs_data()
        .map_err(|_| CoreError::MalformedCobuild)?;
    let cell_deps = raw.cell_deps().map_err(|_| CoreError::MalformedCobuild)?;
    let header_deps = raw.header_deps().map_err(|_| CoreError::MalformedCobuild)?;

    let input_count = inputs.len().map_err(|_| CoreError::MalformedCobuild)?;
    let output_count = outputs.len().map_err(|_| CoreError::MalformedCobuild)?;
    let cell_dep_count = cell_deps.len().map_err(|_| CoreError::MalformedCobuild)?;
    let header_dep_count = header_deps.len().map_err(|_| CoreError::MalformedCobuild)?;

    let mut raw_inputs = Vec::with_capacity(input_count);
    for index in 0..input_count {
        raw_inputs.push(cursor_bytes(
            &inputs
                .get(index)
                .map_err(|_| CoreError::MalformedCobuild)?
                .cursor,
        )?);
    }
    let mut raw_outputs = Vec::with_capacity(output_count);
    for index in 0..output_count {
        raw_outputs.push(cursor_bytes(
            &outputs
                .get(index)
                .map_err(|_| CoreError::MalformedCobuild)?
                .cursor,
        )?);
    }
    let mut raw_outputs_data = Vec::with_capacity(output_count);
    for index in 0..output_count {
        let data = outputs_data
            .get(index)
            .map_err(|_| CoreError::MalformedCobuild)?;
        raw_outputs_data.push(cursor_bytes(&data)?);
    }
    let mut raw_cell_deps = Vec::with_capacity(cell_dep_count);
    for index in 0..cell_dep_count {
        raw_cell_deps.push(cursor_bytes(
            &cell_deps
                .get(index)
                .map_err(|_| CoreError::MalformedCobuild)?
                .cursor,
        )?);
    }
    let mut raw_header_deps = Vec::with_capacity(header_dep_count);
    for index in 0..header_dep_count {
        raw_header_deps.push(
            header_deps
                .get(index)
                .map_err(|_| CoreError::MalformedCobuild)?,
        );
    }

    Ok(TransactionInfo {
        transaction: data.to_vec(),
        witnesses,
        raw_inputs,
        raw_outputs,
        raw_outputs_data,
        raw_cell_deps,
        raw_header_deps,
        input_count,
        output_count,
        cell_dep_count,
        header_dep_count,
    })
}

pub fn script_args_from_slice(data: &[u8]) -> Result<Vec<u8>, CoreError> {
    let script = Script::from(cursor_from_slice(data));
    script
        .verify(false)
        .map_err(|_| CoreError::InvalidOtxLayout)?;
    script
        .args()
        .and_then(TryInto::try_into)
        .map_err(|_| CoreError::MalformedCobuild)
}

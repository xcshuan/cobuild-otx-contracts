use alloc::vec::Vec;
use core::convert::TryInto;

use cobuild_types::lazy_reader::blockchain::{Script, Transaction};

use crate::{
    context::{CobuildContext, PreparedContext, TxScriptHashes},
    error::CoreError,
    hash::{RawTxParts, ResolvedInputHashPart, TxHashParts},
    layout::LayoutTx,
    view::{cursor_bytes, cursor_from_slice},
};

pub struct TransactionInfo {
    pub witnesses: Vec<Vec<u8>>,
    pub raw_parts: RawTxParts,
    pub input_count: usize,
    pub output_count: usize,
    pub cell_dep_count: usize,
    pub header_dep_count: usize,
}

pub struct PreparedContextInput {
    pub witnesses: Vec<Vec<u8>>,
    pub input_count: usize,
    pub output_count: usize,
    pub cell_dep_count: usize,
    pub header_dep_count: usize,
    pub input_locks: Vec<[u8; 32]>,
    pub input_types: Vec<Option<[u8; 32]>>,
    pub output_types: Vec<Option<[u8; 32]>>,
    pub tx_hash: [u8; 32],
    pub resolved_inputs: Vec<ResolvedInputHashPart>,
    pub trailing_witnesses: Vec<Vec<u8>>,
    pub raw_parts: RawTxParts,
}

pub fn prepare_context(input: PreparedContextInput) -> Result<PreparedContext, CoreError> {
    let context = CobuildContext::with_raw_parts(
        LayoutTx {
            witnesses: input.witnesses,
            input_count: input.input_count,
            output_count: input.output_count,
            cell_dep_count: input.cell_dep_count,
            header_dep_count: input.header_dep_count,
        },
        TxScriptHashes {
            input_locks: input.input_locks,
            input_types: input.input_types,
            output_types: input.output_types,
        },
        input.raw_parts,
    )?;
    let hash_parts = TxHashParts {
        tx_hash: input.tx_hash,
        resolved_inputs: input.resolved_inputs,
        trailing_witnesses: input.trailing_witnesses,
    };

    Ok(PreparedContext::new(context, hash_parts))
}

pub fn parse_transaction_info(data: &[u8]) -> Result<TransactionInfo, CoreError> {
    let tx = Transaction::from(cursor_from_slice(data));
    tx.verify(false).map_err(|_| CoreError::InvalidLayout)?;
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
        witnesses,
        raw_parts: RawTxParts {
            inputs: raw_inputs,
            outputs: raw_outputs,
            outputs_data: raw_outputs_data,
            cell_deps: raw_cell_deps,
            header_deps: raw_header_deps,
        },
        input_count,
        output_count,
        cell_dep_count,
        header_dep_count,
    })
}

pub fn script_args_from_slice(data: &[u8]) -> Result<Vec<u8>, CoreError> {
    let script = Script::from(cursor_from_slice(data));
    script.verify(false).map_err(|_| CoreError::InvalidLayout)?;
    script
        .args()
        .and_then(TryInto::try_into)
        .map_err(|_| CoreError::MalformedCobuild)
}

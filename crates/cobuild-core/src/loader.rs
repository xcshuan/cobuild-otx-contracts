use alloc::vec::Vec;
use core::convert::TryInto;

use cobuild_types::lazy_reader::blockchain::{Script, Transaction};

use crate::{
    context::{CobuildContext, PreparedContext, TxScriptHashes},
    error::CoreError,
    hash::{ResolvedInputHashPart, TxHashParts},
    layout::LayoutTx,
    view::cursor_from_slice,
};

pub struct TransactionInfo {
    pub witnesses: Vec<Vec<u8>>,
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
}

pub fn prepare_context(input: PreparedContextInput) -> Result<PreparedContext, CoreError> {
    let context = CobuildContext::new(
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

    Ok(TransactionInfo {
        witnesses,
        input_count: raw
            .inputs()
            .and_then(|inputs| inputs.len())
            .map_err(|_| CoreError::MalformedCobuild)?,
        output_count: raw
            .outputs()
            .and_then(|outputs| outputs.len())
            .map_err(|_| CoreError::MalformedCobuild)?,
        cell_dep_count: raw
            .cell_deps()
            .and_then(|cell_deps| cell_deps.len())
            .map_err(|_| CoreError::MalformedCobuild)?,
        header_dep_count: raw
            .header_deps()
            .and_then(|header_deps| header_deps.len())
            .map_err(|_| CoreError::MalformedCobuild)?,
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

use alloc::vec::Vec;

use crate::{
    context::ScriptHashIndex,
    error::CoreError,
    layout::{scan_layout, LayoutTx, OtxLayoutScan},
    plan::{LockValidationPlan, TypeValidationPlan},
    reader::cursor_bytes_with_error,
    source::{HashInputSource, TxCounts},
};

pub struct CobuildEngine;

pub struct PreparedCobuild {
    pub counts: TxCounts,
    pub script_hashes: ScriptHashIndex,
    pub witnesses: Vec<Vec<u8>>,
    pub layout_scan: OtxLayoutScan,
}

impl CobuildEngine {
    pub fn prepare<S: HashInputSource>(source: &S) -> Result<PreparedCobuild, CoreError> {
        let counts = source.counts()?;
        let script_hashes = script_hashes_from_source(source, counts)?;
        let witnesses = witnesses_from_source(source, counts.witnesses)?;
        let layout_scan = scan_layout(&LayoutTx {
            witnesses: witnesses.clone(),
            input_count: counts.inputs,
            output_count: counts.outputs,
            cell_dep_count: counts.cell_deps,
            header_dep_count: counts.header_deps,
        });

        Ok(PreparedCobuild {
            counts,
            script_hashes,
            witnesses,
            layout_scan,
        })
    }
}

impl PreparedCobuild {
    pub fn counts(&self) -> TxCounts {
        self.counts
    }

    pub fn plan_lock_validation<S: HashInputSource>(
        &self,
        lock_script_hash: [u8; 32],
        _source: &S,
    ) -> Result<LockValidationPlan, CoreError> {
        let _first_input =
            crate::flow::first_input_with_lock(&self.script_hashes, lock_script_hash);

        Ok(LockValidationPlan {
            lock_script_hash,
            required_signatures: Vec::new(),
        })
    }

    pub fn plan_type_validation<S: HashInputSource>(
        &self,
        type_script_hash: [u8; 32],
        _source: &S,
    ) -> Result<TypeValidationPlan, CoreError> {
        Ok(TypeValidationPlan {
            type_script_hash,
            related_messages: Vec::new(),
        })
    }
}

fn script_hashes_from_source<S: HashInputSource>(
    source: &S,
    counts: TxCounts,
) -> Result<ScriptHashIndex, CoreError> {
    let mut input_locks = Vec::with_capacity(counts.inputs);
    let mut input_types = Vec::with_capacity(counts.inputs);
    for index in 0..counts.inputs {
        input_locks.push(source.input_lock_hash(index)?);
        input_types.push(source.input_type_hash(index)?);
    }

    let mut output_types = Vec::with_capacity(counts.outputs);
    for index in 0..counts.outputs {
        output_types.push(source.output_type_hash(index)?);
    }

    Ok(ScriptHashIndex {
        input_locks,
        input_types,
        output_types,
    })
}

fn witnesses_from_source<S: HashInputSource>(
    source: &S,
    witness_count: usize,
) -> Result<Vec<Vec<u8>>, CoreError> {
    let mut witnesses = Vec::with_capacity(witness_count);
    for index in 0..witness_count {
        let witness = source.witness_cursor(index)?;
        witnesses.push(cursor_bytes_with_error(
            &witness.cursor,
            witness.read_error(),
        )?);
    }
    Ok(witnesses)
}

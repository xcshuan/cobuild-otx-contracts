use alloc::vec::Vec;

use crate::{
    context::ScriptHashIndex,
    error::CoreError,
    hash::{tx_with_message_hash, tx_without_message_hash},
    layout::{scan_layout, LayoutTx, OtxLayoutScan},
    message::validate_message_targets,
    plan::{LockValidationPlan, SignatureOrigin, SigningRequirement, TypeValidationPlan},
    reader::{cursor_bytes, cursor_bytes_with_error},
    source::{HashInputSource, TxCounts},
    view::{SighashAllWitnessView, WitnessLayoutView},
};

pub struct CobuildEngine;

pub struct PreparedCobuild {
    pub(crate) counts: TxCounts,
    pub(crate) script_hashes: ScriptHashIndex,
    pub(crate) witnesses: Vec<Vec<u8>>,
    pub(crate) layout_scan: OtxLayoutScan,
}

impl CobuildEngine {
    pub fn prepare<S: HashInputSource>(source: &S) -> Result<PreparedCobuild, CoreError> {
        let counts = source.counts()?;
        let script_hashes = script_hashes_from_source(source, counts)?;
        let tx = LayoutTx {
            witnesses: witnesses_from_source(source, counts.witnesses)?,
            input_count: counts.inputs,
            output_count: counts.outputs,
            cell_dep_count: counts.cell_deps,
            header_dep_count: counts.header_deps,
        };
        let layout_scan = scan_layout(&tx);
        let LayoutTx { witnesses, .. } = tx;

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
        source: &S,
    ) -> Result<LockValidationPlan, CoreError> {
        let required_signatures = self.tx_level_lock_requirements(lock_script_hash, source)?;

        Ok(LockValidationPlan {
            lock_script_hash,
            required_signatures,
        })
    }

    pub fn plan_type_validation<S: HashInputSource>(
        &self,
        type_script_hash: [u8; 32],
        _source: &S,
    ) -> Result<TypeValidationPlan, CoreError> {
        let _prepared_layout = (&self.witnesses, &self.layout_scan);

        Ok(TypeValidationPlan {
            type_script_hash,
            related_messages: Vec::new(),
        })
    }

    fn tx_level_lock_requirements<S: HashInputSource>(
        &self,
        lock_script_hash: [u8; 32],
        source: &S,
    ) -> Result<Vec<SigningRequirement>, CoreError> {
        let Some(carrier_witness_index) =
            crate::flow::first_input_with_lock(&self.script_hashes, lock_script_hash)
        else {
            return Ok(Vec::new());
        };

        let Some(witness) = self.witnesses.get(carrier_witness_index) else {
            return Ok(Vec::new());
        };
        if witness.is_empty() {
            return Ok(Vec::new());
        }

        let view = WitnessLayoutView::from_slice(witness)?;
        let Some(sighash_all_witness_layout) = view.sighash_all_witness_layout()? else {
            return Ok(Vec::new());
        };

        let tx_message = crate::flow::unique_sighash_all_message(&self.witnesses)?;
        let (seal, signing_message_hash) = match sighash_all_witness_layout {
            SighashAllWitnessView::WithMessage { seal, message } => {
                let message = tx_message.as_ref().unwrap_or(&message);
                validate_message_targets(message, &self.script_hashes)?;
                let signing_message_hash = tx_with_message_hash(message, source)?;
                (cursor_bytes(&seal)?, signing_message_hash)
            }
            SighashAllWitnessView::SealOnly { seal } => {
                let signing_message_hash = match tx_message {
                    Some(message) => {
                        validate_message_targets(&message, &self.script_hashes)?;
                        tx_with_message_hash(&message, source)?
                    }
                    None => tx_without_message_hash(source)?,
                };
                (cursor_bytes(&seal)?, signing_message_hash)
            }
        };

        Ok(alloc::vec![SigningRequirement {
            origin: SignatureOrigin::TxLevel,
            carrier_witness_index,
            seal,
            signing_message_hash,
        }])
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

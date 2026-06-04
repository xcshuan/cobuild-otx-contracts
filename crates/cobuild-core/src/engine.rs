use alloc::vec::Vec;

use cobuild_types::lazy_reader::support::Cursor;

use crate::{
    context::ScriptHashIndex,
    error::CoreError,
    hash::{otx_append_hash, otx_base_hash, tx_with_message_hash, tx_without_message_hash},
    layout::{OtxLayoutCollector, OtxLayoutScan},
    message::validate_message_targets,
    plan::{LockValidationPlan, SignatureOrigin, SigningRequirement, TypeValidationPlan},
    protocol::SealScope,
    reader::{cursor_bytes, cursor_bytes_with_error},
    syscalls::{self, TxCounts, TxCountsCache},
    view::{SighashAllWitnessView, WitnessLayoutView},
};

pub struct CobuildEngine;

pub struct PreparedCobuild {
    pub(crate) counts_cache: TxCountsCache,
    pub(crate) script_hashes: ScriptHashIndex,
    witness_summaries: Vec<WitnessSummary>,
    pub(crate) layout_scan: OtxLayoutScan,
}

#[derive(Clone)]
enum WitnessSummary {
    Empty,
    Other,
    Malformed(CoreError),
    SighashAll { message: Cursor },
    SighashAllOnly,
}

impl CobuildEngine {
    pub fn prepare_from_syscalls() -> Result<PreparedCobuild, CoreError> {
        let counts_cache = TxCountsCache::default();
        let counts = syscalls::counts(&counts_cache)?;
        let script_hashes = script_hashes_from_syscalls(counts)?;
        let mut witness_summaries = Vec::with_capacity(counts.witnesses);
        let mut layout_collector = OtxLayoutCollector::new();
        for index in 0..counts.witnesses {
            let witness = syscalls::witness_cursor(index)?;
            let witness = cursor_bytes_with_error(&witness, CoreError::MissingHashInput)?;
            witness_summaries.push(witness_summary(&witness)?);
            layout_collector.push_witness(&witness);
        }
        let layout_scan = layout_collector.finish(
            counts.inputs,
            counts.outputs,
            counts.cell_deps,
            counts.header_deps,
        );

        Ok(PreparedCobuild {
            counts_cache,
            script_hashes,
            witness_summaries,
            layout_scan,
        })
    }
}

impl PreparedCobuild {
    pub fn plan_lock_validation(
        &self,
        lock_script_hash: [u8; 32],
    ) -> Result<LockValidationPlan, CoreError> {
        let mut required_signatures = self.tx_level_lock_requirements(lock_script_hash)?;

        match &self.layout_scan {
            OtxLayoutScan::None => {}
            OtxLayoutScan::Invalid { anchor, error } => {
                let relevance_known_irrelevant = anchor
                    .as_ref()
                    .map(|anchor| {
                        !self
                            .script_hashes
                            .input_locks
                            .iter()
                            .skip(anchor.start_input_cell)
                            .any(|hash| *hash == lock_script_hash)
                    })
                    .unwrap_or(false);
                if !relevance_known_irrelevant {
                    return Err(error.clone());
                }
            }
            OtxLayoutScan::Complete(layout) => {
                for otx in &layout.otx_data {
                    let base_relevant = crate::flow::script_in_input_range(
                        &self.script_hashes.input_locks,
                        otx.layout.base_inputs,
                        lock_script_hash,
                    );
                    let append_relevant = crate::flow::script_in_input_range(
                        &self.script_hashes.input_locks,
                        otx.layout.append_inputs,
                        lock_script_hash,
                    );
                    if !base_relevant && !append_relevant {
                        continue;
                    }

                    validate_message_targets(&otx.witness.message, &self.script_hashes)?;
                    let base_hash = otx_base_hash(&otx.witness, &otx.layout, &self.counts_cache)?;
                    if base_relevant {
                        let seal = crate::seal::unique_otx_seal_by_scope(
                            lock_script_hash,
                            &otx.witness.seals,
                            SealScope::Base,
                        )?;
                        required_signatures.push(SigningRequirement {
                            origin: SignatureOrigin::OtxBase,
                            carrier_witness_index: otx.layout.witness_index,
                            seal,
                            signing_message_hash: base_hash,
                        });
                    }
                    if append_relevant {
                        let seal = crate::seal::unique_otx_seal_by_scope(
                            lock_script_hash,
                            &otx.witness.seals,
                            SealScope::Append,
                        )?;
                        required_signatures.push(SigningRequirement {
                            origin: SignatureOrigin::OtxAppend,
                            carrier_witness_index: otx.layout.witness_index,
                            seal,
                            signing_message_hash: otx_append_hash(
                                &otx.witness,
                                &otx.layout,
                                &self.counts_cache,
                                base_hash,
                            )?,
                        });
                    }
                }
            }
        }

        let has_tx_level = required_signatures
            .iter()
            .any(|requirement| requirement.origin == SignatureOrigin::TxLevel);
        let has_otx = required_signatures.iter().any(|requirement| {
            matches!(
                requirement.origin,
                SignatureOrigin::OtxBase | SignatureOrigin::OtxAppend
            )
        });
        if has_otx && !has_tx_level {
            if let OtxLayoutScan::Complete(layout) = &self.layout_scan {
                if !crate::flow::lock_group_fully_covered_by_otx(
                    &self.script_hashes.input_locks,
                    lock_script_hash,
                    &layout.otxs,
                ) {
                    return Err(CoreError::MissingLockGroupCoverage);
                }
            }
        }

        Ok(LockValidationPlan {
            lock_script_hash,
            required_signatures,
        })
    }

    pub fn plan_type_validation(
        &self,
        type_script_hash: [u8; 32],
    ) -> Result<TypeValidationPlan, CoreError> {
        let mut related_messages = Vec::new();

        let tx_level_type_relevant = match &self.layout_scan {
            OtxLayoutScan::Complete(layout) => {
                for (otx_index, otx) in layout.otx_data.iter().enumerate() {
                    let relation = crate::plan::OtxTypeRelation {
                        input_type_in_base: crate::flow::type_hash_in_input_range(
                            &self.script_hashes.input_types,
                            otx.layout.base_inputs,
                            type_script_hash,
                        ),
                        input_type_in_append: crate::flow::type_hash_in_input_range(
                            &self.script_hashes.input_types,
                            otx.layout.append_inputs,
                            type_script_hash,
                        ),
                        output_type_in_base: crate::flow::type_hash_in_output_range(
                            &self.script_hashes.output_types,
                            otx.layout.base_outputs,
                            type_script_hash,
                        ),
                        output_type_in_base_covered:
                            crate::flow::covered_type_hash_in_base_outputs(
                                &self.script_hashes.output_types,
                                otx.layout.base_outputs,
                                type_script_hash,
                                &otx.witness.base_output_masks,
                            )?,
                        output_type_in_append: crate::flow::type_hash_in_output_range(
                            &self.script_hashes.output_types,
                            otx.layout.append_outputs,
                            type_script_hash,
                        ),
                    };
                    let is_related = relation.input_type_in_base
                        || relation.input_type_in_append
                        || relation.output_type_in_base
                        || relation.output_type_in_append;
                    if !is_related {
                        continue;
                    }
                    validate_message_targets(&otx.witness.message, &self.script_hashes)?;
                    related_messages.push(crate::plan::RelatedMessage {
                        origin: crate::plan::MessageOrigin::Otx {
                            witness_index: otx.layout.witness_index,
                            otx_index,
                            layout: crate::plan::OtxMessageLayout {
                                base_inputs: otx.layout.base_inputs,
                                append_inputs: otx.layout.append_inputs,
                                base_outputs: otx.layout.base_outputs,
                                append_outputs: otx.layout.append_outputs,
                                base_cell_deps: otx.layout.base_cell_deps,
                                append_cell_deps: otx.layout.append_cell_deps,
                                base_header_deps: otx.layout.base_header_deps,
                                append_header_deps: otx.layout.append_header_deps,
                            },
                            relation,
                        },
                        message: otx.witness.message.clone().into(),
                    });
                }
                crate::flow::type_hash_outside_otx_ranges(
                    &self.script_hashes.input_types,
                    &self.script_hashes.output_types,
                    type_script_hash,
                    &layout.otxs,
                )
            }
            OtxLayoutScan::Invalid { anchor, error } => {
                let relevance_known_irrelevant = anchor
                    .as_ref()
                    .map(|anchor| {
                        !self
                            .script_hashes
                            .input_types
                            .iter()
                            .skip(anchor.start_input_cell)
                            .any(|hash| *hash == Some(type_script_hash))
                            && !self
                                .script_hashes
                                .output_types
                                .iter()
                                .skip(anchor.start_output_cell)
                                .any(|hash| *hash == Some(type_script_hash))
                    })
                    .unwrap_or(false);
                if !relevance_known_irrelevant {
                    return Err(error.clone());
                }
                crate::flow::type_hash_present(
                    &self.script_hashes.input_types,
                    &self.script_hashes.output_types,
                    type_script_hash,
                )
            }
            OtxLayoutScan::None => crate::flow::type_hash_present(
                &self.script_hashes.input_types,
                &self.script_hashes.output_types,
                type_script_hash,
            ),
        };

        if tx_level_type_relevant {
            if let Some((carrier_witness_index, message)) =
                unique_sighash_all_message_with_index_from_summaries(&self.witness_summaries)?
            {
                validate_message_targets(&message, &self.script_hashes)?;
                related_messages.push(crate::plan::RelatedMessage {
                    origin: crate::plan::MessageOrigin::TxLevel {
                        carrier_witness_index,
                    },
                    message: message.into(),
                });
            }
        }

        Ok(TypeValidationPlan {
            type_script_hash,
            related_messages,
        })
    }

    fn tx_level_lock_requirements(
        &self,
        lock_script_hash: [u8; 32],
    ) -> Result<Vec<SigningRequirement>, CoreError> {
        let Some(carrier_witness_index) =
            crate::flow::first_input_with_lock(&self.script_hashes, lock_script_hash)
        else {
            return Ok(Vec::new());
        };

        match self.witness_summaries.get(carrier_witness_index) {
            Some(WitnessSummary::SighashAll { .. }) | Some(WitnessSummary::SighashAllOnly) => {}
            Some(WitnessSummary::Malformed(error)) => return Err(error.clone()),
            Some(WitnessSummary::Empty | WitnessSummary::Other) | None => return Ok(Vec::new()),
        }

        let carrier = syscalls::witness_cursor(carrier_witness_index)?;
        let carrier_bytes = cursor_bytes_with_error(&carrier, CoreError::MissingHashInput)?;
        let view = WitnessLayoutView::from_slice(&carrier_bytes)?;
        let Some(sighash_all_witness_layout) = view.sighash_all_witness_layout()? else {
            return Ok(Vec::new());
        };

        let tx_message = unique_sighash_all_message_from_summaries(&self.witness_summaries)?;
        let (seal, signing_message_hash) = match sighash_all_witness_layout {
            SighashAllWitnessView::WithMessage { seal, message } => {
                let message = tx_message.as_ref().unwrap_or(&message);
                validate_message_targets(message, &self.script_hashes)?;
                let signing_message_hash = tx_with_message_hash(message, &self.counts_cache)?;
                (cursor_bytes(&seal)?, signing_message_hash)
            }
            SighashAllWitnessView::SealOnly { seal } => {
                let signing_message_hash = match tx_message {
                    Some(message) => {
                        validate_message_targets(&message, &self.script_hashes)?;
                        tx_with_message_hash(&message, &self.counts_cache)?
                    }
                    None => tx_without_message_hash(&self.counts_cache)?,
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

fn script_hashes_from_syscalls(counts: TxCounts) -> Result<ScriptHashIndex, CoreError> {
    let mut input_locks = Vec::with_capacity(counts.inputs);
    let mut input_types = Vec::with_capacity(counts.inputs);
    for index in 0..counts.inputs {
        input_locks.push(syscalls::input_lock_hash(index)?);
        input_types.push(syscalls::input_type_hash(index)?);
    }

    let mut output_types = Vec::with_capacity(counts.outputs);
    for index in 0..counts.outputs {
        output_types.push(syscalls::output_type_hash(index)?);
    }

    Ok(ScriptHashIndex {
        input_locks,
        input_types,
        output_types,
    })
}

fn witness_summary(witness: &[u8]) -> Result<WitnessSummary, CoreError> {
    if witness.is_empty() {
        return Ok(WitnessSummary::Empty);
    }

    let view = match WitnessLayoutView::from_slice(witness) {
        Ok(view) => view,
        Err(error) => {
            return if has_tx_level_witness_id(witness) {
                Ok(WitnessSummary::Malformed(error))
            } else {
                Ok(WitnessSummary::Other)
            };
        }
    };
    if let Some(message) = view.sighash_all_message()? {
        return Ok(WitnessSummary::SighashAll { message });
    }
    if view.is_sighash_all_only() {
        return Ok(WitnessSummary::SighashAllOnly);
    }
    Ok(WitnessSummary::Other)
}

fn has_tx_level_witness_id(witness: &[u8]) -> bool {
    if witness.len() < 4 {
        return false;
    }
    let item_id = u32::from_le_bytes([witness[0], witness[1], witness[2], witness[3]]);
    matches!(item_id, 0xff00_0001 | 0xff00_0002)
}

fn unique_sighash_all_message_from_summaries(
    summaries: &[WitnessSummary],
) -> Result<Option<Cursor>, CoreError> {
    let mut message = None;
    for summary in summaries {
        match summary {
            WitnessSummary::SighashAll { message: candidate } => {
                if message.is_some() {
                    return Err(CoreError::DuplicateSighashAll);
                }
                message = Some(candidate.clone());
            }
            WitnessSummary::Malformed(error) => return Err(error.clone()),
            _ => {}
        }
    }
    Ok(message)
}

fn unique_sighash_all_message_with_index_from_summaries(
    summaries: &[WitnessSummary],
) -> Result<Option<(usize, Cursor)>, CoreError> {
    let mut message = None;
    for (index, summary) in summaries.iter().enumerate() {
        match summary {
            WitnessSummary::SighashAll { message: candidate } => {
                if message.is_some() {
                    return Err(CoreError::DuplicateSighashAll);
                }
                message = Some((index, candidate.clone()));
            }
            WitnessSummary::Malformed(error) => return Err(error.clone()),
            _ => {}
        }
    }
    Ok(message)
}

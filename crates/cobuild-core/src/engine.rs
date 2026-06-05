use alloc::vec::Vec;

use crate::{
    context::TxScriptHashes,
    error::CoreError,
    hash::{otx_append_hash, otx_base_hash, tx_with_message_hash, tx_without_message_hash},
    layout::{OtxLayoutCollector, OtxLayoutScan},
    plan::{LockValidationPlan, SignatureOrigin, SigningRequirement, TypeValidationPlan},
    protocol::SealScope,
    reader::{cursor_bytes, cursor_bytes_with_error},
    syscalls,
    view::{SighashAllWitnessView, WitnessLayoutView},
    witness::WitnessScan,
};

pub struct CobuildEngine;

pub struct PreparedCobuild {
    pub(crate) tx: syscalls::SyscallTxReader,
    pub(crate) script_hashes: TxScriptHashes,
    witnesses: WitnessScan,
    pub(crate) layout_scan: OtxLayoutScan,
}

impl CobuildEngine {
    pub fn prepare_from_syscalls() -> Result<PreparedCobuild, CoreError> {
        let tx = syscalls::SyscallTxReader::default();
        let counts = tx.counts()?;
        let script_hashes = TxScriptHashes::from_reader(&tx)?;
        let mut witnesses = WitnessScan::with_capacity(counts.witnesses);
        let mut layout_collector = OtxLayoutCollector::new();
        for index in 0..counts.witnesses {
            let witness = tx.witness_cursor(index)?;
            let witness = cursor_bytes_with_error(&witness, CoreError::MissingHashInput)?;
            witnesses.push_witness(&witness)?;
            layout_collector.push_witness(&witness);
        }
        let layout_scan = layout_collector.finish(
            counts.inputs,
            counts.outputs,
            counts.cell_deps,
            counts.header_deps,
        );

        Ok(PreparedCobuild {
            tx,
            script_hashes,
            witnesses,
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
                    let base_relevant = self
                        .script_hashes
                        .lock_in_input_range(otx.layout.base_inputs, lock_script_hash);
                    let append_relevant = self
                        .script_hashes
                        .lock_in_input_range(otx.layout.append_inputs, lock_script_hash);
                    if !base_relevant && !append_relevant {
                        continue;
                    }

                    self.script_hashes
                        .validate_message_targets(&otx.witness.message)?;
                    let base_hash = otx_base_hash(&otx.witness, &otx.layout, &self.tx)?;
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
                                &self.tx,
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
                if !self
                    .script_hashes
                    .lock_group_fully_covered_by_otx(lock_script_hash, &layout.otxs)
                {
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
                    let relation = self
                        .script_hashes
                        .type_relation_for_otx(otx, type_script_hash)?;
                    let is_related = relation.input_type_in_base
                        || relation.input_type_in_append
                        || relation.output_type_in_base
                        || relation.output_type_in_append;
                    if !is_related {
                        continue;
                    }
                    self.script_hashes
                        .validate_message_targets(&otx.witness.message)?;
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
                self.script_hashes
                    .type_hash_outside_otx_ranges(type_script_hash, &layout.otxs)
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
                self.script_hashes.type_hash_present(type_script_hash)
            }
            OtxLayoutScan::None => self.script_hashes.type_hash_present(type_script_hash),
        };

        if tx_level_type_relevant {
            if let Some((carrier_witness_index, message)) =
                self.witnesses.unique_sighash_all_message_with_index()?
            {
                self.script_hashes.validate_message_targets(&message)?;
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
            self.script_hashes.first_input_with_lock(lock_script_hash)
        else {
            return Ok(Vec::new());
        };

        if !self
            .witnesses
            .tx_level_carrier_has_sighash_all_layout(carrier_witness_index)?
        {
            return Ok(Vec::new());
        }

        let carrier = self.tx.witness_cursor(carrier_witness_index)?;
        let carrier_bytes = cursor_bytes_with_error(&carrier, CoreError::MissingHashInput)?;
        let view = WitnessLayoutView::from_slice(&carrier_bytes)?;
        let Some(sighash_all_witness_layout) = view.sighash_all_witness_layout()? else {
            return Ok(Vec::new());
        };

        let tx_message = self.witnesses.unique_sighash_all_message()?;
        let (seal, signing_message_hash) = match sighash_all_witness_layout {
            SighashAllWitnessView::WithMessage { seal, message } => {
                let message = tx_message.as_ref().unwrap_or(&message);
                self.script_hashes.validate_message_targets(message)?;
                let signing_message_hash = tx_with_message_hash(message, &self.tx)?;
                (cursor_bytes(&seal)?, signing_message_hash)
            }
            SighashAllWitnessView::SealOnly { seal } => {
                let signing_message_hash = match tx_message {
                    Some(message) => {
                        self.script_hashes.validate_message_targets(&message)?;
                        tx_with_message_hash(&message, &self.tx)?
                    }
                    None => tx_without_message_hash(&self.tx)?,
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

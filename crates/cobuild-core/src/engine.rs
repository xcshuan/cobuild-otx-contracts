use alloc::vec::Vec;

use crate::{
    context::TxScriptHashes,
    error::CoreError,
    hash::{otx_append_hash, otx_base_hash, tx_with_message_hash, tx_without_message_hash},
    layout::{OtxLayoutCollector, OtxLayoutScan},
    plan::{
        LockValidationPlan, MessageOrigin, OtxMessageLayout, RelatedMessage, SignatureOrigin,
        SigningRequirement, TypeRelatedMessage, TypeValidationPlan,
    },
    protocol::SealScope,
    reader::{cursor_bytes, cursor_bytes_with_error},
    syscalls::SyscallTxReader,
    view::{SighashAllWitnessView, WitnessLayoutView},
    witness::WitnessScan,
};

pub struct CobuildContext {
    pub(crate) tx: SyscallTxReader,
    pub(crate) script_hashes: TxScriptHashes,
    witnesses: WitnessScan,
    pub(crate) layout_scan: OtxLayoutScan,
}

impl CobuildContext {
    pub fn from_syscalls() -> Result<Self, CoreError> {
        let mut tx = SyscallTxReader::default();
        tx.preload_counts_from_syscalls()?;
        let script_hashes = TxScriptHashes::from_reader(&tx)?;
        let counts = tx.counts();
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

        Ok(Self {
            tx,
            script_hashes,
            witnesses,
            layout_scan,
        })
    }

    pub fn plan_lock_validation(
        &self,
        lock_script_hash: [u8; 32],
    ) -> Result<LockValidationPlan, CoreError> {
        LockPlanBuilder::new(self, lock_script_hash).build()
    }

    pub fn plan_type_validation(
        &self,
        type_script_hash: [u8; 32],
    ) -> Result<TypeValidationPlan, CoreError> {
        TypePlanBuilder::new(self, type_script_hash).build()
    }
}

struct LockPlanBuilder<'a> {
    context: &'a CobuildContext,
    lock_script_hash: [u8; 32],
    required_signatures: Vec<SigningRequirement>,
}

impl<'a> LockPlanBuilder<'a> {
    fn new(context: &'a CobuildContext, lock_script_hash: [u8; 32]) -> Self {
        Self {
            context,
            lock_script_hash,
            required_signatures: Vec::new(),
        }
    }

    fn build(mut self) -> Result<LockValidationPlan, CoreError> {
        self.add_tx_level_requirement()?;
        self.add_otx_requirements()?;
        self.ensure_otx_lock_group_coverage()?;
        Ok(LockValidationPlan {
            lock_script_hash: self.lock_script_hash,
            required_signatures: self.required_signatures,
            related_messages: Vec::new(),
        })
    }

    fn add_tx_level_requirement(&mut self) -> Result<(), CoreError> {
        let Some(carrier_witness_index) = self
            .context
            .script_hashes
            .first_input_with_lock(self.lock_script_hash)
        else {
            return Ok(());
        };

        if !self
            .context
            .witnesses
            .tx_level_carrier_has_sighash_all_layout(carrier_witness_index)?
        {
            return Ok(());
        }

        let carrier = self.context.tx.witness_cursor(carrier_witness_index)?;
        let carrier_bytes = cursor_bytes_with_error(&carrier, CoreError::MissingHashInput)?;
        let view = WitnessLayoutView::from_slice(&carrier_bytes)?;
        let Some(sighash_all_witness_layout) = view.sighash_all_witness_layout()? else {
            return Ok(());
        };

        let tx_message = self.context.witnesses.unique_sighash_all_message()?;
        let (seal, signing_message_hash) = match sighash_all_witness_layout {
            SighashAllWitnessView::WithMessage { seal, message } => {
                let message = tx_message.as_ref().unwrap_or(&message);
                self.context
                    .script_hashes
                    .validate_message_targets(message)?;
                let signing_message_hash = tx_with_message_hash(message, &self.context.tx)?;
                (cursor_bytes(&seal)?, signing_message_hash)
            }
            SighashAllWitnessView::SealOnly { seal } => {
                let signing_message_hash = match tx_message {
                    Some(message) => {
                        self.context
                            .script_hashes
                            .validate_message_targets(&message)?;
                        tx_with_message_hash(&message, &self.context.tx)?
                    }
                    None => tx_without_message_hash(&self.context.tx)?,
                };
                (cursor_bytes(&seal)?, signing_message_hash)
            }
        };

        self.required_signatures.push(SigningRequirement {
            origin: SignatureOrigin::TxLevel,
            carrier_witness_index,
            seal,
            signing_message_hash,
        });

        Ok(())
    }

    fn add_otx_requirements(&mut self) -> Result<(), CoreError> {
        match &self.context.layout_scan {
            OtxLayoutScan::None => {}
            OtxLayoutScan::Invalid { anchor, error } => {
                let relevance_known_irrelevant = anchor
                    .as_ref()
                    .map(|anchor| {
                        !self
                            .context
                            .script_hashes
                            .input_locks
                            .iter()
                            .skip(anchor.start_input_cell)
                            .any(|hash| *hash == self.lock_script_hash)
                    })
                    .unwrap_or(false);
                if !relevance_known_irrelevant {
                    return Err(error.clone());
                }
            }
            OtxLayoutScan::Complete(layout) => {
                for otx in &layout.otx_entries {
                    let base_relevant = self
                        .context
                        .script_hashes
                        .lock_in_input_range(otx.layout.base_inputs, self.lock_script_hash);
                    let append_relevant = self
                        .context
                        .script_hashes
                        .lock_in_input_range(otx.layout.append_inputs, self.lock_script_hash);
                    if !base_relevant && !append_relevant {
                        continue;
                    }

                    self.context
                        .script_hashes
                        .validate_message_targets(&otx.witness.message)?;
                    let base_hash = otx_base_hash(&otx.witness, &otx.layout, &self.context.tx)?;
                    if base_relevant {
                        let seal = crate::seal::unique_otx_seal_by_scope(
                            self.lock_script_hash,
                            &otx.witness.seals,
                            SealScope::Base,
                        )?;
                        self.required_signatures.push(SigningRequirement {
                            origin: SignatureOrigin::OtxBase,
                            carrier_witness_index: otx.layout.witness_index,
                            seal,
                            signing_message_hash: base_hash,
                        });
                    }
                    if append_relevant {
                        let seal = crate::seal::unique_otx_seal_by_scope(
                            self.lock_script_hash,
                            &otx.witness.seals,
                            SealScope::Append,
                        )?;
                        self.required_signatures.push(SigningRequirement {
                            origin: SignatureOrigin::OtxAppend,
                            carrier_witness_index: otx.layout.witness_index,
                            seal,
                            signing_message_hash: otx_append_hash(
                                &otx.witness,
                                &otx.layout,
                                &self.context.tx,
                                base_hash,
                            )?,
                        });
                    }
                }
            }
        }

        Ok(())
    }

    fn ensure_otx_lock_group_coverage(&self) -> Result<(), CoreError> {
        let has_tx_level = self
            .required_signatures
            .iter()
            .any(|requirement| requirement.origin == SignatureOrigin::TxLevel);
        let has_otx = self.required_signatures.iter().any(|requirement| {
            matches!(
                requirement.origin,
                SignatureOrigin::OtxBase | SignatureOrigin::OtxAppend
            )
        });
        if has_otx && !has_tx_level {
            if let OtxLayoutScan::Complete(layout) = &self.context.layout_scan {
                if !self
                    .context
                    .script_hashes
                    .lock_group_fully_covered_by_otx(self.lock_script_hash, &layout.otxs)
                {
                    return Err(CoreError::MissingLockGroupCoverage);
                }
            }
        }

        Ok(())
    }
}

struct TypePlanBuilder<'a> {
    context: &'a CobuildContext,
    type_script_hash: [u8; 32],
    related_messages: Vec<TypeRelatedMessage>,
}

impl<'a> TypePlanBuilder<'a> {
    fn new(context: &'a CobuildContext, type_script_hash: [u8; 32]) -> Self {
        Self {
            context,
            type_script_hash,
            related_messages: Vec::new(),
        }
    }

    fn build(mut self) -> Result<TypeValidationPlan, CoreError> {
        let tx_level_type_relevant = self.add_otx_related_messages()?;
        self.add_tx_level_message_if_relevant(tx_level_type_relevant)?;
        Ok(TypeValidationPlan {
            type_script_hash: self.type_script_hash,
            related_messages: self.related_messages,
        })
    }

    fn add_otx_related_messages(&mut self) -> Result<bool, CoreError> {
        match &self.context.layout_scan {
            OtxLayoutScan::Complete(layout) => {
                for (otx_index, otx) in layout.otx_entries.iter().enumerate() {
                    let relation = self
                        .context
                        .script_hashes
                        .type_relation_for_otx(otx, self.type_script_hash)?;
                    let is_related = relation.input_type_in_base
                        || relation.input_type_in_append
                        || relation.output_type_in_base
                        || relation.output_type_in_append;
                    if !is_related {
                        continue;
                    }
                    self.context
                        .script_hashes
                        .validate_message_targets(&otx.witness.message)?;
                    self.related_messages.push(TypeRelatedMessage {
                        message: RelatedMessage {
                            origin: MessageOrigin::Otx {
                                witness_index: otx.layout.witness_index,
                                otx_index,
                                layout: OtxMessageLayout {
                                    base_inputs: otx.layout.base_inputs,
                                    append_inputs: otx.layout.append_inputs,
                                    base_outputs: otx.layout.base_outputs,
                                    append_outputs: otx.layout.append_outputs,
                                    base_cell_deps: otx.layout.base_cell_deps,
                                    append_cell_deps: otx.layout.append_cell_deps,
                                    base_header_deps: otx.layout.base_header_deps,
                                    append_header_deps: otx.layout.append_header_deps,
                                },
                            },
                            message: otx.witness.message.clone().into(),
                        },
                        otx_relation: Some(relation),
                    });
                }
                Ok(self
                    .context
                    .script_hashes
                    .type_hash_outside_otx_ranges(self.type_script_hash, &layout.otxs))
            }
            OtxLayoutScan::Invalid { anchor, error } => {
                self.tx_level_type_relevant_from_invalid_layout(anchor.as_ref(), error)
            }
            OtxLayoutScan::None => Ok(self
                .context
                .script_hashes
                .type_hash_present(self.type_script_hash)),
        }
    }

    fn tx_level_type_relevant_from_invalid_layout(
        &self,
        anchor: Option<&crate::view::OtxStartView>,
        error: &CoreError,
    ) -> Result<bool, CoreError> {
        let relevance_known_irrelevant = anchor
            .map(|anchor| {
                !self
                    .context
                    .script_hashes
                    .input_types
                    .iter()
                    .skip(anchor.start_input_cell)
                    .any(|hash| *hash == Some(self.type_script_hash))
                    && !self
                        .context
                        .script_hashes
                        .output_types
                        .iter()
                        .skip(anchor.start_output_cell)
                        .any(|hash| *hash == Some(self.type_script_hash))
            })
            .unwrap_or(false);
        if !relevance_known_irrelevant {
            return Err(error.clone());
        }
        Ok(self
            .context
            .script_hashes
            .type_hash_present(self.type_script_hash))
    }

    fn add_tx_level_message_if_relevant(
        &mut self,
        tx_level_type_relevant: bool,
    ) -> Result<(), CoreError> {
        if tx_level_type_relevant {
            if let Some((carrier_witness_index, message)) = self
                .context
                .witnesses
                .unique_sighash_all_message_with_index()?
            {
                self.context
                    .script_hashes
                    .validate_message_targets(&message)?;
                self.related_messages.push(TypeRelatedMessage {
                    message: RelatedMessage {
                        origin: MessageOrigin::TxLevel {
                            carrier_witness_index,
                        },
                        message: message.into(),
                    },
                    otx_relation: None,
                });
            }
        }
        Ok(())
    }
}

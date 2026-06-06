use alloc::vec::Vec;

use cobuild_types::lazy_reader::support::Cursor;

use crate::{
    context::{CurrentScript, CurrentScriptContext},
    error::CoreError,
    hash::{otx_append_hash, otx_base_hash, tx_with_message_hash, tx_without_message_hash},
    layout::OtxLayoutScan,
    plan::{
        LockValidationPlan, MessageOrigin, OtxMessageLayout, RelatedMessage, SignatureOrigin,
        SigningRequirement, TypeRelatedMessage, TypeValidationPlan,
    },
    protocol::{ScriptRole, SealScope},
    reader::cursor_bytes,
    syscalls::SyscallTxReader,
    view::MessageView,
    witness::{CobuildWitnessScanner, TxLevelCarrierView, WitnessScan},
};

pub struct CobuildContext {
    pub(crate) tx: SyscallTxReader,
    pub(crate) script_context: CurrentScriptContext,
    witnesses: WitnessScan,
    pub(crate) layout_scan: OtxLayoutScan,
}

impl CobuildContext {
    pub fn build(current_script: CurrentScript) -> Result<Self, CoreError> {
        let tx = SyscallTxReader::from_syscalls()?;
        let script_context = CurrentScriptContext::from_reader(&tx, current_script)?;
        let counts = tx.counts();
        let mut scanner = CobuildWitnessScanner::with_capacity(counts.witnesses);
        for index in 0..counts.witnesses {
            let witness = tx.witness_cursor(index)?;
            scanner.push_witness(witness)?;
        }
        let scanned = scanner.finish(
            counts.inputs,
            counts.outputs,
            counts.cell_deps,
            counts.header_deps,
        )?;

        Ok(Self {
            tx,
            script_context,
            witnesses: scanned.tx_level,
            layout_scan: scanned.otx_layout,
        })
    }

    pub fn plan_lock_validation(&self) -> Result<LockValidationPlan, CoreError> {
        LockPlanBuilder::new(self)?.build()
    }

    pub fn plan_type_validation(&self) -> Result<TypeValidationPlan, CoreError> {
        TypePlanBuilder::new(self)?.build()
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;

    fn test_otx(
        message_bytes: &[u8],
        base_start: usize,
        append_start: usize,
    ) -> crate::layout::OtxLayoutEntry {
        crate::layout::OtxLayoutEntry {
            layout: crate::layout::OtxLayout {
                witness_index: 7,
                base_inputs: crate::layout::Range {
                    start: base_start,
                    count: 1,
                },
                append_inputs: crate::layout::Range {
                    start: append_start,
                    count: 1,
                },
                base_outputs: crate::layout::Range { start: 0, count: 1 },
                append_outputs: crate::layout::Range { start: 1, count: 0 },
                base_cell_deps: crate::layout::Range { start: 0, count: 0 },
                append_cell_deps: crate::layout::Range { start: 0, count: 0 },
                base_header_deps: crate::layout::Range { start: 0, count: 0 },
                append_header_deps: crate::layout::Range { start: 0, count: 0 },
            },
            witness: crate::view::OtxView {
                message: crate::reader::cursor_from_slice(message_bytes),
                append_permissions: 0,
                base_input_cells: 1,
                base_input_masks: crate::view::MaskView::new(vec![0]),
                base_output_cells: 0,
                base_output_masks: crate::view::MaskView::new(Vec::new()),
                base_cell_deps: 0,
                base_cell_dep_masks: crate::view::MaskView::new(Vec::new()),
                base_header_deps: 0,
                base_header_dep_masks: crate::view::MaskView::new(Vec::new()),
                append_input_cells: 0,
                append_output_cells: 0,
                append_cell_deps: 0,
                append_header_deps: 0,
                seals: Vec::new(),
            },
        }
    }

    #[test]
    fn lock_related_tx_message_preserves_origin_and_message_cursor() {
        let message_bytes = [4u8, 0, 0, 0];
        let message = crate::reader::cursor_from_slice(&message_bytes);
        let related = related_tx_message(2, message.clone());

        assert!(matches!(
            related.origin,
            MessageOrigin::TxLevel {
                carrier_witness_index: 2
            }
        ));
        assert_eq!(
            crate::reader::cursor_bytes(related.message.cursor()).unwrap(),
            message_bytes.to_vec()
        );
    }

    #[test]
    fn lock_related_otx_message_preserves_origin_layout_and_message_cursor() {
        let message_bytes = [4u8, 0, 0, 0];
        let otx = test_otx(&message_bytes, 1, 3);

        let related = related_otx_message(3, &otx);

        match related.origin {
            MessageOrigin::Otx {
                witness_index,
                otx_index,
                layout,
            } => {
                assert_eq!(witness_index, 7);
                assert_eq!(otx_index, 3);
                assert_eq!(layout.base_inputs.start, 1);
                assert_eq!(layout.append_inputs.start, 3);
            }
            MessageOrigin::TxLevel { .. } => panic!("expected OTX message origin"),
        }
        assert_eq!(
            crate::reader::cursor_bytes(related.message.cursor()).unwrap(),
            message_bytes.to_vec()
        );
    }
}

struct LockPlanBuilder<'a> {
    context: &'a CobuildContext,
    lock_script_hash: [u8; 32],
    required_signatures: Vec<SigningRequirement>,
    related_messages: Vec<RelatedMessage>,
}

struct LockOtxRelevance {
    base_signature: bool,
    append_signature: bool,
}

impl LockOtxRelevance {
    fn needs_signature(&self) -> bool {
        self.base_signature || self.append_signature
    }
}

impl<'a> LockPlanBuilder<'a> {
    fn new(context: &'a CobuildContext) -> Result<Self, CoreError> {
        Ok(Self {
            context,
            lock_script_hash: context.script_context.current_lock_hash()?,
            required_signatures: Vec::new(),
            related_messages: Vec::new(),
        })
    }

    fn build(mut self) -> Result<LockValidationPlan, CoreError> {
        self.add_tx_level_requirement()?;
        self.add_otx_requirements()?;
        self.ensure_otx_lock_group_coverage()?;
        Ok(LockValidationPlan {
            lock_script_hash: self.lock_script_hash,
            required_signatures: self.required_signatures,
            related_messages: self.related_messages,
        })
    }

    fn add_tx_level_requirement(&mut self) -> Result<(), CoreError> {
        let current_lock_inputs = self.current_lock_inputs()?;
        let Some(carrier_witness_index) = current_lock_inputs.first().copied() else {
            return Ok(());
        };

        if !self.context.witnesses.has_cobuild_witness_layout() {
            return Ok(());
        }
        if !self.tx_level_remainder_exists()? {
            return Ok(());
        }
        if !self
            .context
            .witnesses
            .tx_level_carrier_has_sighash_all_layout(carrier_witness_index)?
        {
            return Err(CoreError::InvalidLockGroupWitness);
        }
        self.context.witnesses.ensure_non_carrier_witnesses_empty(
            current_lock_inputs.iter().copied(),
            carrier_witness_index,
        )?;

        let Some(carrier) = self
            .context
            .witnesses
            .tx_level_carrier_view(carrier_witness_index)?
        else {
            return Err(CoreError::InvalidLockGroupWitness);
        };

        let tx_message = self.context.witnesses.unique_sighash_all_message()?;
        let mut related_message = None;
        let (seal, signing_message_hash) = match carrier {
            TxLevelCarrierView::WithMessage { seal, message } => {
                let message = tx_message.as_ref().unwrap_or(&message);
                self.context
                    .script_context
                    .validate_message_targets(message)?;
                related_message = Some(message.clone());
                let signing_message_hash = tx_with_message_hash(message, &self.context.tx)?;
                (cursor_bytes(&seal)?, signing_message_hash)
            }
            TxLevelCarrierView::SealOnly { seal } => {
                let signing_message_hash = match tx_message {
                    Some(message) => {
                        self.context
                            .script_context
                            .validate_message_targets(&message)?;
                        related_message = Some(message.clone());
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

        self.collect_tx_related_message(carrier_witness_index, related_message);

        Ok(())
    }

    fn tx_level_remainder_exists(&self) -> Result<bool, CoreError> {
        match &self.context.layout_scan {
            OtxLayoutScan::None => Ok(!self.current_lock_inputs()?.is_empty()),
            OtxLayoutScan::Complete(layout) => self
                .context
                .script_context
                .current_lock_outside_otx_ranges(&layout.otx_entries),
        }
    }

    fn current_lock_inputs(&self) -> Result<&[usize], CoreError> {
        self.context.script_context.current_lock_inputs()
    }

    fn add_otx_requirements(&mut self) -> Result<(), CoreError> {
        match &self.context.layout_scan {
            OtxLayoutScan::None => {}
            OtxLayoutScan::Complete(layout) => {
                for (otx_index, otx) in layout.otx_entries.iter().enumerate() {
                    let Some(relevance) =
                        self.collect_otx_related_message_if_relevant(otx_index, otx)?
                    else {
                        continue;
                    };

                    self.context
                        .script_context
                        .validate_message_targets(&otx.witness.message)?;
                    if !relevance.needs_signature() {
                        continue;
                    }

                    let base_hash = otx_base_hash(&otx.witness, &otx.layout, &self.context.tx)?;
                    if relevance.base_signature {
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
                    if relevance.append_signature {
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

    fn collect_tx_related_message(
        &mut self,
        carrier_witness_index: usize,
        message: Option<Cursor>,
    ) {
        if let Some(message) = message {
            self.related_messages
                .push(related_tx_message(carrier_witness_index, message));
        }
    }

    fn collect_otx_related_message_if_relevant(
        &mut self,
        otx_index: usize,
        otx: &crate::layout::OtxLayoutEntry,
    ) -> Result<Option<LockOtxRelevance>, CoreError> {
        let base_signature = self
            .context
            .script_context
            .input_range_contains_current_lock(otx.layout.base_inputs)?;
        let append_signature = self
            .context
            .script_context
            .input_range_contains_current_lock(otx.layout.append_inputs)?;
        let scope_related = base_signature || append_signature;
        let action_related = if scope_related {
            false
        } else {
            message_targets_lock_script(&otx.witness.message, self.lock_script_hash)?
        };
        if !scope_related && !action_related {
            return Ok(None);
        }

        self.related_messages
            .push(related_otx_message(otx_index, otx));
        Ok(Some(LockOtxRelevance {
            base_signature,
            append_signature,
        }))
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
                    .script_context
                    .all_current_lock_inputs_covered_by_otx(&layout.otx_entries)?
                {
                    return Err(CoreError::MissingLockGroupCoverage);
                }
            }
        }

        Ok(())
    }
}

fn related_tx_message(carrier_witness_index: usize, message: Cursor) -> RelatedMessage {
    RelatedMessage {
        origin: MessageOrigin::TxLevel {
            carrier_witness_index,
        },
        message: message.into(),
    }
}

fn related_otx_message(otx_index: usize, otx: &crate::layout::OtxLayoutEntry) -> RelatedMessage {
    RelatedMessage {
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
    }
}

struct TypePlanBuilder<'a> {
    context: &'a CobuildContext,
    type_script_hash: [u8; 32],
    related_messages: Vec<TypeRelatedMessage>,
}

impl<'a> TypePlanBuilder<'a> {
    fn new(context: &'a CobuildContext) -> Result<Self, CoreError> {
        Ok(Self {
            context,
            type_script_hash: context.script_context.current_type_hash()?,
            related_messages: Vec::new(),
        })
    }

    fn build(mut self) -> Result<TypeValidationPlan, CoreError> {
        self.add_otx_related_messages()?;
        self.add_tx_level_message_if_relevant()?;
        Ok(TypeValidationPlan {
            type_script_hash: self.type_script_hash,
            related_messages: self.related_messages,
        })
    }

    fn add_otx_related_messages(&mut self) -> Result<(), CoreError> {
        match &self.context.layout_scan {
            OtxLayoutScan::Complete(layout) => {
                for (otx_index, otx) in layout.otx_entries.iter().enumerate() {
                    let relation = self.context.script_context.type_relation_for_otx(otx)?;
                    let range_related = otx_type_relation_mentions_type(&relation);
                    let action_related = if range_related {
                        false
                    } else {
                        message_targets_type_script(&otx.witness.message, self.type_script_hash)?
                    };
                    if !range_related && !action_related {
                        continue;
                    }
                    self.context
                        .script_context
                        .validate_message_targets(&otx.witness.message)?;
                    self.related_messages.push(TypeRelatedMessage {
                        message: related_otx_message(otx_index, otx),
                        otx_relation: Some(relation),
                    });
                }
                Ok(())
            }
            OtxLayoutScan::None => Ok(()),
        }
    }

    fn add_tx_level_message_if_relevant(&mut self) -> Result<(), CoreError> {
        let Some((carrier_witness_index, message)) = self
            .context
            .witnesses
            .unique_sighash_all_message_with_index()?
        else {
            return Ok(());
        };

        let scope_related = self.tx_level_scope_mentions_type()?;
        let action_related = if scope_related {
            false
        } else {
            message_targets_type_script(&message, self.type_script_hash)?
        };
        if !scope_related && !action_related {
            return Ok(());
        }

        self.context
            .script_context
            .validate_message_targets(&message)?;
        self.related_messages.push(TypeRelatedMessage {
            message: related_tx_message(carrier_witness_index, message),
            otx_relation: None,
        });
        Ok(())
    }

    fn tx_level_scope_mentions_type(&self) -> Result<bool, CoreError> {
        match &self.context.layout_scan {
            OtxLayoutScan::Complete(layout) => self
                .context
                .script_context
                .current_type_outside_otx_ranges(&layout.otx_entries),
            OtxLayoutScan::None => self.context.script_context.current_type_present(),
        }
    }
}

fn otx_type_relation_mentions_type(relation: &crate::plan::OtxTypeRelation) -> bool {
    relation.input_type_in_base
        || relation.input_type_in_append
        || relation.output_type_in_base
        || relation.output_type_in_append
}

fn message_targets_lock_script(
    message: &Cursor,
    lock_script_hash: [u8; 32],
) -> Result<bool, CoreError> {
    Ok(MessageView::new(message.clone())
        .actions()?
        .into_iter()
        .any(|action| {
            action.script_hash == lock_script_hash && action.script_role == ScriptRole::InputLock
        }))
}

fn message_targets_type_script(
    message: &Cursor,
    type_script_hash: [u8; 32],
) -> Result<bool, CoreError> {
    Ok(MessageView::new(message.clone())
        .actions()?
        .into_iter()
        .any(|action| {
            action.script_hash == type_script_hash
                && matches!(
                    action.script_role,
                    ScriptRole::InputType | ScriptRole::OutputType
                )
        }))
}

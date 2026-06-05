use alloc::vec::Vec;

use cobuild_types::lazy_reader::support::Cursor;

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

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;

    fn hash(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn test_context(input_locks: Vec<[u8; 32]>) -> CobuildContext {
        CobuildContext {
            tx: SyscallTxReader::default(),
            script_hashes: TxScriptHashes {
                input_locks,
                input_types: Vec::new(),
                output_types: Vec::new(),
            },
            witnesses: WitnessScan::with_capacity(0),
            layout_scan: OtxLayoutScan::None,
        }
    }

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

    #[test]
    fn lock_builder_collects_tx_related_message_only_when_present() {
        let lock_hash = hash(1);
        let context = test_context(vec![lock_hash]);
        let mut builder = LockPlanBuilder::new(&context, lock_hash);
        let message_bytes = [4u8, 0, 0, 0];

        builder.collect_tx_related_message(2, None);
        assert!(builder.related_messages.is_empty());

        builder
            .collect_tx_related_message(2, Some(crate::reader::cursor_from_slice(&message_bytes)));

        assert_eq!(builder.related_messages.len(), 1);
        assert!(matches!(
            builder.related_messages[0].origin,
            MessageOrigin::TxLevel {
                carrier_witness_index: 2
            }
        ));
        assert_eq!(
            crate::reader::cursor_bytes(builder.related_messages[0].message.cursor()).unwrap(),
            message_bytes.to_vec()
        );
    }

    #[test]
    fn lock_builder_collects_one_otx_related_message_for_base_and_append_relevance() {
        let lock_hash = hash(1);
        let other_hash = hash(2);
        let context = test_context(vec![lock_hash, other_hash, lock_hash]);
        let mut builder = LockPlanBuilder::new(&context, lock_hash);
        let message_bytes = [4u8, 0, 0, 0];
        let otx = test_otx(&message_bytes, 0, 2);

        let (base_relevant, append_relevant) = builder
            .collect_otx_related_message_if_relevant(5, &otx)
            .unwrap();

        assert!(base_relevant);
        assert!(append_relevant);
        assert_eq!(builder.related_messages.len(), 1);
        match builder.related_messages[0].origin {
            MessageOrigin::Otx { otx_index, .. } => assert_eq!(otx_index, 5),
            MessageOrigin::TxLevel { .. } => panic!("expected OTX message origin"),
        }
    }

    #[test]
    fn lock_builder_skips_irrelevant_otx_related_message() {
        let lock_hash = hash(1);
        let other_hash = hash(2);
        let context = test_context(vec![other_hash, other_hash]);
        let mut builder = LockPlanBuilder::new(&context, lock_hash);
        let message_bytes = [4u8, 0, 0, 0];
        let otx = test_otx(&message_bytes, 0, 1);

        assert!(builder
            .collect_otx_related_message_if_relevant(0, &otx)
            .is_none());
        assert!(builder.related_messages.is_empty());
    }
}

struct LockPlanBuilder<'a> {
    context: &'a CobuildContext,
    lock_script_hash: [u8; 32],
    required_signatures: Vec<SigningRequirement>,
    related_messages: Vec<RelatedMessage>,
}

impl<'a> LockPlanBuilder<'a> {
    fn new(context: &'a CobuildContext, lock_script_hash: [u8; 32]) -> Self {
        Self {
            context,
            lock_script_hash,
            required_signatures: Vec::new(),
            related_messages: Vec::new(),
        }
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
        let mut related_message = None;
        let (seal, signing_message_hash) = match sighash_all_witness_layout {
            SighashAllWitnessView::WithMessage { seal, message } => {
                let message = tx_message.as_ref().unwrap_or(&message);
                self.context
                    .script_hashes
                    .validate_message_targets(message)?;
                related_message = Some(message.clone());
                let signing_message_hash = tx_with_message_hash(message, &self.context.tx)?;
                (cursor_bytes(&seal)?, signing_message_hash)
            }
            SighashAllWitnessView::SealOnly { seal } => {
                let signing_message_hash = match tx_message {
                    Some(message) => {
                        self.context
                            .script_hashes
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
                for (otx_index, otx) in layout.otx_entries.iter().enumerate() {
                    let Some((base_relevant, append_relevant)) =
                        self.collect_otx_related_message_if_relevant(otx_index, otx)
                    else {
                        continue;
                    };

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
    ) -> Option<(bool, bool)> {
        let base_relevant = self
            .context
            .script_hashes
            .lock_in_input_range(otx.layout.base_inputs, self.lock_script_hash);
        let append_relevant = self
            .context
            .script_hashes
            .lock_in_input_range(otx.layout.append_inputs, self.lock_script_hash);
        if !base_relevant && !append_relevant {
            return None;
        }

        self.related_messages
            .push(related_otx_message(otx_index, otx));
        Some((base_relevant, append_relevant))
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

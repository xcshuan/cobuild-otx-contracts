use alloc::vec::Vec;

use cobuild_types::lazy_reader::support::Cursor;

use crate::{
    context::{CurrentScript, CurrentScriptContext},
    error::CoreError,
    hash::{otx_append_segment_hash, otx_base_hash, tx_with_message_hash, tx_without_message_hash},
    layout::OtxLayouts,
    plan::{
        ActionOrigin, ActionRef, LockValidationPlan, OtxMessageLayout, RelatedAction,
        SignatureOrigin, SigningRequirement, TypeActionOtxScope, TypeRelatedAction,
        TypeValidationPlan,
    },
    protocol::ScriptRole,
    reader::cursor_bytes,
    syscalls::SyscallTxReader,
    view::{ActionView, MessageView},
    witness::{CobuildWitnessScanner, TxLevelCarrierView, WitnessScan},
};

pub struct CobuildContext {
    pub(crate) tx: SyscallTxReader,
    pub(crate) script_context: CurrentScriptContext,
    witnesses: WitnessScan,
    pub(crate) otx_layouts: OtxLayouts,
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
            otx_layouts: scanned.otx_layouts,
        })
    }

    pub fn plan_lock_validation(&self) -> Result<LockValidationPlan, CoreError> {
        LockPlanBuilder::new(self)?.build()
    }

    pub fn plan_type_validation(&self) -> Result<TypeValidationPlan, CoreError> {
        TypePlanBuilder::new(self)?.build()
    }

    pub fn otx_actions(&self, otx_index: usize) -> Result<Vec<RelatedAction>, CoreError> {
        let otx = self.otx_entry(otx_index)?;
        let actions = checked_message_actions(self, &otx.witness.message)?;
        Ok(actions
            .into_iter()
            .map(|action| related_otx_action(otx_index, otx, action))
            .collect())
    }

    pub fn tx_level_actions(&self) -> Result<Option<Vec<RelatedAction>>, CoreError> {
        let Some((witness_index, message)) =
            self.witnesses.unique_sighash_all_message_with_index()?
        else {
            return Ok(None);
        };

        let actions = checked_message_actions(self, &message)?;
        Ok(Some(
            actions
                .into_iter()
                .map(|action| related_tx_action(witness_index, action))
                .collect(),
        ))
    }

    pub fn find_action(&self, action_ref: ActionRef) -> Result<RelatedAction, CoreError> {
        match action_ref {
            ActionRef::TxLevel {
                witness_index,
                action_index,
            } => {
                let Some((actual_witness_index, message)) =
                    self.witnesses.unique_sighash_all_message_with_index()?
                else {
                    return Err(CoreError::ActionNotFound);
                };
                if actual_witness_index != witness_index {
                    return Err(CoreError::ActionNotFound);
                }
                let action = message_action(&message, action_index)?;
                Ok(related_tx_action(witness_index, action))
            }
            ActionRef::Otx {
                witness_index,
                otx_index,
                action_index,
            } => {
                let otx = self.otx_entry_for_action_ref(otx_index)?;
                if otx.layout.witness_index != witness_index {
                    return Err(CoreError::ActionNotFound);
                }
                let action = message_action(&otx.witness.message, action_index)?;
                Ok(related_otx_action(otx_index, otx, action))
            }
        }
    }

    pub fn all_actions(&self) -> Result<Vec<RelatedAction>, CoreError> {
        let mut actions = Vec::new();
        if let Some(tx_level_actions) = self.tx_level_actions()? {
            actions.extend(tx_level_actions);
        }
        if let OtxLayouts::Complete(layout) = &self.otx_layouts {
            for otx_index in 0..layout.otx_entries.len() {
                actions.extend(self.otx_actions(otx_index)?);
            }
        }
        Ok(actions)
    }

    fn otx_entry(&self, otx_index: usize) -> Result<&crate::layout::OtxLayoutEntry, CoreError> {
        let OtxLayouts::Complete(layout) = &self.otx_layouts else {
            return Err(CoreError::InvalidOtxLayout);
        };
        layout
            .otx_entries
            .get(otx_index)
            .ok_or(CoreError::InvalidOtxLayout)
    }

    fn otx_entry_for_action_ref(
        &self,
        otx_index: usize,
    ) -> Result<&crate::layout::OtxLayoutEntry, CoreError> {
        let OtxLayouts::Complete(layout) = &self.otx_layouts else {
            return Err(CoreError::ActionNotFound);
        };
        layout
            .otx_entries
            .get(otx_index)
            .ok_or(CoreError::ActionNotFound)
    }
}

fn checked_message_actions(
    context: &CobuildContext,
    message: &Cursor,
) -> Result<Vec<ActionView>, CoreError> {
    let actions = message_actions(message)?;
    context.script_context.validate_action_targets(&actions)?;
    Ok(actions)
}

fn message_actions(message: &Cursor) -> Result<Vec<ActionView>, CoreError> {
    MessageView::new(message.clone()).actions_from_verified_message()
}

fn message_action(message: &Cursor, action_index: usize) -> Result<ActionView, CoreError> {
    MessageView::new(message.clone())
        .action_from_verified_message(action_index)?
        .ok_or(CoreError::ActionNotFound)
}

fn lock_actions_from_actions(
    actions: &[ActionView],
    lock_script_hash: [u8; 32],
) -> Vec<ActionView> {
    actions
        .iter()
        .filter(|action| {
            action.script_role == ScriptRole::InputLock && action.script_hash == lock_script_hash
        })
        .cloned()
        .collect()
}

fn type_actions_from_actions(
    actions: &[ActionView],
    type_script_hash: [u8; 32],
) -> Vec<ActionView> {
    actions
        .iter()
        .filter(|action| {
            matches!(
                action.script_role,
                ScriptRole::InputType | ScriptRole::OutputType
            ) && action.script_hash == type_script_hash
        })
        .cloned()
        .collect()
}

struct LockPlanBuilder<'a> {
    context: &'a CobuildContext,
    lock_script_hash: [u8; 32],
    required_signatures: Vec<SigningRequirement>,
    related_actions: Vec<RelatedAction>,
    tx_level_action_cache: Option<TxLevelActionCache>,
}

struct TxLevelActionCache {
    witness_index: usize,
    actions: Vec<ActionView>,
    targets_checked: bool,
}

impl<'a> LockPlanBuilder<'a> {
    fn new(context: &'a CobuildContext) -> Result<Self, CoreError> {
        Ok(Self {
            context,
            lock_script_hash: context.script_context.current_lock_hash()?,
            required_signatures: Vec::new(),
            related_actions: Vec::new(),
            tx_level_action_cache: None,
        })
    }

    fn build(mut self) -> Result<LockValidationPlan, CoreError> {
        self.add_tx_level_requirement()?;
        self.add_tx_level_actions()?;
        self.add_otx_requirements()?;
        self.ensure_otx_lock_group_coverage()?;
        Ok(LockValidationPlan {
            lock_script_hash: self.lock_script_hash,
            required_signatures: self.required_signatures,
            related_actions: self.related_actions,
        })
    }

    fn add_tx_level_requirement(&mut self) -> Result<(), CoreError> {
        if !self.context.witnesses.has_cobuild_witness_layout() {
            return Ok(());
        }
        if !self.current_lock_needs_tx_level_signature()? {
            return Ok(());
        }

        let current_lock_inputs = self.current_lock_inputs()?;
        let carrier_witness_index = current_lock_inputs
            .first()
            .copied()
            .ok_or(CoreError::InvalidContextInput)?;

        self.context.witnesses.ensure_non_carrier_witnesses_empty(
            current_lock_inputs.iter().copied(),
            carrier_witness_index,
        )?;

        let carrier = self
            .context
            .witnesses
            .tx_level_carrier_view(carrier_witness_index)?;

        let tx_message = self
            .context
            .witnesses
            .unique_sighash_all_message_with_index()?;
        let (seal, signing_message_hash) = match carrier {
            TxLevelCarrierView::WithMessage { seal, message } => {
                let (message_witness_index, message) = tx_message
                    .as_ref()
                    .map(|(witness_index, message)| (*witness_index, message))
                    .unwrap_or((carrier_witness_index, &message));
                self.checked_tx_level_message_actions(message_witness_index, message)?;
                let signing_message_hash = tx_with_message_hash(message, &self.context.tx)?;
                (cursor_bytes(&seal)?, signing_message_hash)
            }
            TxLevelCarrierView::SealOnly { seal } => {
                let signing_message_hash = match tx_message {
                    Some((message_witness_index, message)) => {
                        self.checked_tx_level_message_actions(message_witness_index, &message)?;
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

    fn add_tx_level_actions(&mut self) -> Result<(), CoreError> {
        let Some((witness_index, message)) = self
            .context
            .witnesses
            .unique_sighash_all_message_with_index()?
        else {
            return Ok(());
        };

        self.add_tx_related_actions(witness_index, &message)?;
        Ok(())
    }

    fn add_tx_related_actions(
        &mut self,
        witness_index: usize,
        message: &Cursor,
    ) -> Result<(), CoreError> {
        let all_actions = self.tx_level_message_actions(witness_index, message)?;
        let actions = lock_actions_from_actions(&all_actions, self.lock_script_hash);
        if actions.is_empty() {
            return Ok(());
        }

        self.ensure_tx_level_message_targets_checked(witness_index, &all_actions)?;
        self.related_actions.extend(
            actions
                .into_iter()
                .map(|action| related_tx_action(witness_index, action)),
        );
        Ok(())
    }

    fn current_lock_needs_tx_level_signature(&self) -> Result<bool, CoreError> {
        let current_lock_inputs = self.current_lock_inputs()?;
        if current_lock_inputs.is_empty() {
            return Ok(false);
        }

        match &self.context.otx_layouts {
            OtxLayouts::None => Ok(true),
            // Only the current lock group matters here. Other-lock inputs may be
            // outside the aggregate OTX input range and are validated by their
            // own lock scripts.
            OtxLayouts::Complete(layout) => self
                .context
                .script_context
                .current_lock_has_inputs_outside_range(layout.input_range),
        }
    }

    fn current_lock_inputs(&self) -> Result<&[usize], CoreError> {
        self.context.script_context.current_lock_inputs()
    }

    fn tx_level_message_actions(
        &mut self,
        witness_index: usize,
        message: &Cursor,
    ) -> Result<Vec<ActionView>, CoreError> {
        if let Some(cache) = &self.tx_level_action_cache {
            if cache.witness_index == witness_index {
                return Ok(cache.actions.clone());
            }
        }

        let actions = message_actions(message)?;
        self.tx_level_action_cache = Some(TxLevelActionCache {
            witness_index,
            actions: actions.clone(),
            targets_checked: false,
        });
        Ok(actions)
    }

    fn checked_tx_level_message_actions(
        &mut self,
        witness_index: usize,
        message: &Cursor,
    ) -> Result<Vec<ActionView>, CoreError> {
        let actions = self.tx_level_message_actions(witness_index, message)?;
        self.ensure_tx_level_message_targets_checked(witness_index, &actions)?;
        Ok(actions)
    }

    fn ensure_tx_level_message_targets_checked(
        &mut self,
        witness_index: usize,
        actions: &[ActionView],
    ) -> Result<(), CoreError> {
        if self
            .tx_level_action_cache
            .as_ref()
            .is_some_and(|cache| cache.witness_index == witness_index && cache.targets_checked)
        {
            return Ok(());
        }

        self.context
            .script_context
            .validate_action_targets(actions)?;
        if let Some(cache) = &mut self.tx_level_action_cache {
            if cache.witness_index == witness_index {
                cache.targets_checked = true;
            }
        }
        Ok(())
    }

    fn add_otx_requirements(&mut self) -> Result<(), CoreError> {
        let OtxLayouts::Complete(layout) = &self.context.otx_layouts else {
            return Ok(());
        };

        for (otx_index, otx) in layout.otx_entries.iter().enumerate() {
            self.add_otx_requirement(otx_index, otx)?;
        }

        Ok(())
    }

    fn add_otx_requirement(
        &mut self,
        otx_index: usize,
        otx: &crate::layout::OtxLayoutEntry,
    ) -> Result<(), CoreError> {
        let base_signature = self
            .context
            .script_context
            .input_range_contains_current_lock(otx.layout.base_inputs)?;
        let append_segment_indices = self.required_append_segment_indices(otx)?;
        let all_actions = message_actions(&otx.witness.message)?;
        let actions = lock_actions_from_actions(&all_actions, self.lock_script_hash);
        let needs_signature = base_signature || !append_segment_indices.is_empty();
        if !needs_signature && actions.is_empty() {
            return Ok(());
        }

        self.context
            .script_context
            .validate_action_targets(&all_actions)?;
        self.push_otx_actions(otx_index, otx, actions);
        if needs_signature {
            self.add_otx_signatures(otx, base_signature, &append_segment_indices)?;
        }

        Ok(())
    }

    fn required_append_segment_indices(
        &self,
        otx: &crate::layout::OtxLayoutEntry,
    ) -> Result<Vec<usize>, CoreError> {
        let mut append_segment_indices = Vec::new();
        for (segment_index, segment) in otx.layout.append_segments.iter().enumerate() {
            if self
                .context
                .script_context
                .input_range_contains_current_lock(segment.inputs)?
            {
                append_segment_indices.push(segment_index);
            }
        }
        Ok(append_segment_indices)
    }

    fn push_otx_actions(
        &mut self,
        otx_index: usize,
        otx: &crate::layout::OtxLayoutEntry,
        actions: Vec<ActionView>,
    ) {
        for action in actions {
            self.related_actions
                .push(related_otx_action(otx_index, otx, action));
        }
    }

    fn add_otx_signatures(
        &mut self,
        otx: &crate::layout::OtxLayoutEntry,
        base_signature: bool,
        append_segment_indices: &[usize],
    ) -> Result<(), CoreError> {
        let base_hash = otx_base_hash(&otx.witness, &otx.layout, &self.context.tx)?;
        if base_signature {
            let seal =
                crate::seal::unique_lock_seal(self.lock_script_hash, &otx.witness.base_seals)?;
            self.required_signatures.push(SigningRequirement {
                origin: SignatureOrigin::OtxBase,
                carrier_witness_index: otx.layout.witness_index,
                seal,
                signing_message_hash: base_hash,
            });
        }
        for &segment_index in append_segment_indices {
            let segment = otx
                .witness
                .append_segments
                .get(segment_index)
                .ok_or(CoreError::InvalidOtxLayout)?;
            let seal = crate::seal::unique_lock_seal(self.lock_script_hash, &segment.seals)?;
            self.required_signatures.push(SigningRequirement {
                origin: SignatureOrigin::OtxAppendSegment { segment_index },
                carrier_witness_index: otx.layout.witness_index,
                seal,
                signing_message_hash: otx_append_segment_hash(
                    &otx.witness,
                    &otx.layout,
                    segment_index,
                    &self.context.tx,
                    base_hash,
                )?,
            });
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
                SignatureOrigin::OtxBase | SignatureOrigin::OtxAppendSegment { .. }
            )
        });
        // If this lock uses OTX-scoped signatures without a tx-level signature,
        // every input in the current lock group must be covered by the aggregate
        // OTX input range. This intentionally ignores inputs from other lock
        // groups. In full contract execution, missing or malformed tx-level
        // carrier witnesses can fail earlier with InvalidLockGroupWitness.
        if has_otx && !has_tx_level {
            if let OtxLayouts::Complete(layout) = &self.context.otx_layouts {
                if !self
                    .context
                    .script_context
                    .all_current_lock_inputs_in_range(layout.input_range)?
                {
                    return Err(CoreError::MissingLockGroupCoverage);
                }
            }
        }

        Ok(())
    }
}

fn related_tx_action(witness_index: usize, action: ActionView) -> RelatedAction {
    RelatedAction {
        origin: ActionOrigin::TxLevel { witness_index },
        action,
    }
}

fn related_otx_action(
    otx_index: usize,
    otx: &crate::layout::OtxLayoutEntry,
    action: ActionView,
) -> RelatedAction {
    RelatedAction {
        origin: ActionOrigin::Otx {
            witness_index: otx.layout.witness_index,
            otx_index,
            layout: OtxMessageLayout {
                base_inputs: otx.layout.base_inputs,
                append_inputs: otx.layout.append_inputs(),
                base_outputs: otx.layout.base_outputs,
                append_outputs: otx.layout.append_outputs(),
                base_cell_deps: otx.layout.base_cell_deps,
                append_cell_deps: otx.layout.append_cell_deps(),
                base_header_deps: otx.layout.base_header_deps,
                append_header_deps: otx.layout.append_header_deps(),
            },
        },
        action,
    }
}

struct TypePlanBuilder<'a> {
    context: &'a CobuildContext,
    type_script_hash: [u8; 32],
    related_actions: Vec<TypeRelatedAction>,
}

impl<'a> TypePlanBuilder<'a> {
    fn new(context: &'a CobuildContext) -> Result<Self, CoreError> {
        Ok(Self {
            context,
            type_script_hash: context.script_context.current_type_hash()?,
            related_actions: Vec::new(),
        })
    }

    fn build(mut self) -> Result<TypeValidationPlan, CoreError> {
        self.add_otx_actions()?;
        self.add_tx_level_actions()?;
        Ok(TypeValidationPlan {
            type_script_hash: self.type_script_hash,
            related_actions: self.related_actions,
        })
    }

    fn add_otx_actions(&mut self) -> Result<(), CoreError> {
        let OtxLayouts::Complete(layout) = &self.context.otx_layouts else {
            return Ok(());
        };

        for (otx_index, otx) in layout.otx_entries.iter().enumerate() {
            self.add_otx_action(otx_index, otx)?;
        }

        Ok(())
    }

    fn add_otx_action(
        &mut self,
        otx_index: usize,
        otx: &crate::layout::OtxLayoutEntry,
    ) -> Result<(), CoreError> {
        let relation = self.context.script_context.type_relation_for_otx(otx)?;
        let scope_related = otx_type_relation_mentions_type(&relation);
        let all_actions = message_actions(&otx.witness.message)?;
        let actions = type_actions_from_actions(&all_actions, self.type_script_hash);
        if !scope_related && actions.is_empty() {
            return Ok(());
        }

        self.context
            .script_context
            .validate_action_targets(&all_actions)?;
        self.push_otx_actions(otx_index, otx, actions, type_action_otx_scope(relation));
        Ok(())
    }

    fn push_otx_actions(
        &mut self,
        otx_index: usize,
        otx: &crate::layout::OtxLayoutEntry,
        actions: Vec<ActionView>,
        otx_type_scope: TypeActionOtxScope,
    ) {
        for action in actions {
            self.related_actions.push(TypeRelatedAction {
                action: related_otx_action(otx_index, otx, action),
                otx_type_scope,
            });
        }
    }

    fn add_tx_level_actions(&mut self) -> Result<(), CoreError> {
        let Some((witness_index, message)) = self
            .context
            .witnesses
            .unique_sighash_all_message_with_index()?
        else {
            return Ok(());
        };

        self.add_tx_related_actions(witness_index, &message)?;
        Ok(())
    }

    fn add_tx_related_actions(
        &mut self,
        witness_index: usize,
        message: &Cursor,
    ) -> Result<(), CoreError> {
        let scope_related = self.tx_level_scope_mentions_type()?;
        let all_actions = message_actions(message)?;
        let actions = type_actions_from_actions(&all_actions, self.type_script_hash);
        if !scope_related && actions.is_empty() {
            return Ok(());
        }

        self.context
            .script_context
            .validate_action_targets(&all_actions)?;
        self.push_tx_related_actions(witness_index, actions);
        Ok(())
    }

    fn push_tx_related_actions(&mut self, witness_index: usize, actions: Vec<ActionView>) {
        for action in actions {
            self.related_actions.push(TypeRelatedAction {
                action: related_tx_action(witness_index, action),
                otx_type_scope: TypeActionOtxScope::TargetOnly,
            });
        }
    }

    fn tx_level_scope_mentions_type(&self) -> Result<bool, CoreError> {
        match &self.context.otx_layouts {
            OtxLayouts::Complete(layout) => self
                .context
                .script_context
                .current_type_outside_ranges(layout.input_range, layout.output_range),
            OtxLayouts::None => self.context.script_context.current_type_present(),
        }
    }
}

fn type_action_otx_scope(relation: crate::plan::OtxTypeRelation) -> TypeActionOtxScope {
    if otx_type_relation_mentions_type(&relation) {
        TypeActionOtxScope::InOtxScope(relation)
    } else {
        TypeActionOtxScope::TargetOnly
    }
}

fn otx_type_relation_mentions_type(relation: &crate::plan::OtxTypeRelation) -> bool {
    relation.input_type_in_base
        || relation.input_type_in_append
        || relation.output_type_in_base
        || relation.output_type_in_append
}

#[cfg(test)]
fn lock_actions_for_message(
    message: &Cursor,
    lock_script_hash: [u8; 32],
) -> Result<Vec<ActionView>, CoreError> {
    MessageView::new(message.clone()).actions_for(ScriptRole::InputLock, lock_script_hash)
}

#[cfg(test)]
fn type_actions_for_message(
    message: &Cursor,
    type_script_hash: [u8; 32],
) -> Result<Vec<ActionView>, CoreError> {
    let view = MessageView::new(message.clone());
    let mut actions = view.actions_for(ScriptRole::InputType, type_script_hash)?;
    actions.extend(view.actions_for(ScriptRole::OutputType, type_script_hash)?);
    Ok(actions)
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
                base_outputs: crate::layout::Range { start: 0, count: 1 },
                base_cell_deps: crate::layout::Range { start: 0, count: 0 },
                base_header_deps: crate::layout::Range { start: 0, count: 0 },
                append_segments: vec![crate::layout::OtxAppendSegmentLayout {
                    flags: crate::protocol::SegmentFlags::try_from(0).unwrap(),
                    inputs: crate::layout::Range {
                        start: append_start,
                        count: 1,
                    },
                    outputs: crate::layout::Range { start: 1, count: 0 },
                    cell_deps: crate::layout::Range { start: 0, count: 0 },
                    header_deps: crate::layout::Range { start: 0, count: 0 },
                }],
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
                append_segments: vec![crate::view::OtxAppendSegmentView {
                    segment_flags: 0,
                    input_cells: 1,
                    output_cells: 0,
                    cell_deps: 0,
                    header_deps: 0,
                    seals: Vec::new(),
                }],
                base_seals: Vec::new(),
            },
        }
    }

    fn test_action(role: ScriptRole, script_hash: [u8; 32], data: &[u8]) -> ActionView {
        ActionView {
            index: 0,
            script_info_hash: [0x33; 32],
            script_role: role,
            script_hash,
            data: crate::reader::cursor_from_slice(data),
        }
    }

    fn test_context_with_otx_entries(
        otx_entries: Vec<crate::layout::OtxLayoutEntry>,
    ) -> CobuildContext {
        CobuildContext {
            tx: crate::syscalls::SyscallTxReader::from_cached_parts_for_tests(
                crate::syscalls::TxCounts::default(),
                crate::reader::cursor_from_slice(&empty_transaction()),
                [0u8; 32],
            ),
            script_context: crate::context::CurrentScriptContext::from_script_for_tests(
                CurrentScript::InputLock([0u8; 32]),
            ),
            witnesses: crate::witness::WitnessScan::with_capacity(0),
            otx_layouts: crate::layout::OtxLayouts::Complete(crate::layout::BuiltLayout {
                input_range: crate::layout::Range { start: 0, count: 0 },
                output_range: crate::layout::Range { start: 0, count: 0 },
                otx_entries,
            }),
        }
    }

    fn lock_context_with_otx_entries(
        current_lock_hash: [u8; 32],
        input_locks: Vec<[u8; 32]>,
        otx_entries: Vec<crate::layout::OtxLayoutEntry>,
    ) -> CobuildContext {
        CobuildContext {
            tx: crate::syscalls::SyscallTxReader::from_cached_parts_for_tests(
                crate::syscalls::TxCounts::default(),
                crate::reader::cursor_from_slice(&empty_transaction()),
                [0u8; 32],
            ),
            script_context: crate::context::CurrentScriptContext::input_lock_for_tests(
                current_lock_hash,
                input_locks,
            ),
            witnesses: crate::witness::WitnessScan::with_capacity(0),
            otx_layouts: crate::layout::OtxLayouts::Complete(crate::layout::BuiltLayout {
                input_range: crate::layout::Range { start: 0, count: 0 },
                output_range: crate::layout::Range { start: 0, count: 0 },
                otx_entries,
            }),
        }
    }

    fn lock_context_with_tx_message_and_otx_entries(
        current_lock_hash: [u8; 32],
        input_locks: Vec<[u8; 32]>,
        tx_message: &[u8],
        otx_entries: Vec<crate::layout::OtxLayoutEntry>,
    ) -> CobuildContext {
        let tx_witness = sighash_all_witness_bytes(&[0x99], tx_message);
        CobuildContext {
            tx: crate::syscalls::SyscallTxReader::from_cached_parts_for_tests(
                crate::syscalls::TxCounts::default(),
                crate::reader::cursor_from_slice(&empty_transaction()),
                [0u8; 32],
            ),
            script_context: crate::context::CurrentScriptContext::input_lock_for_tests(
                current_lock_hash,
                input_locks,
            ),
            witnesses: scan_tx_level_witnesses(&[tx_witness]),
            otx_layouts: crate::layout::OtxLayouts::Complete(crate::layout::BuiltLayout {
                input_range: crate::layout::Range { start: 0, count: 0 },
                output_range: crate::layout::Range { start: 0, count: 0 },
                otx_entries,
            }),
        }
    }

    fn lock_context_with_input_range(
        current_lock_hash: [u8; 32],
        input_locks: Vec<[u8; 32]>,
        input_range: crate::layout::Range,
    ) -> CobuildContext {
        CobuildContext {
            tx: crate::syscalls::SyscallTxReader::from_cached_parts_for_tests(
                crate::syscalls::TxCounts::default(),
                crate::reader::cursor_from_slice(&empty_transaction()),
                [0u8; 32],
            ),
            script_context: crate::context::CurrentScriptContext::input_lock_for_tests(
                current_lock_hash,
                input_locks,
            ),
            witnesses: crate::witness::WitnessScan::with_capacity(0),
            otx_layouts: crate::layout::OtxLayouts::Complete(crate::layout::BuiltLayout {
                input_range,
                output_range: crate::layout::Range { start: 0, count: 0 },
                otx_entries: Vec::new(),
            }),
        }
    }

    fn lock_context_with_tx_message(
        current_lock_hash: [u8; 32],
        input_locks: Vec<[u8; 32]>,
        tx_message: &[u8],
    ) -> CobuildContext {
        let tx_witness = sighash_all_witness_bytes(&[0x99], tx_message);
        CobuildContext {
            tx: crate::syscalls::SyscallTxReader::from_cached_parts_for_tests(
                crate::syscalls::TxCounts::default(),
                crate::reader::cursor_from_slice(&empty_transaction()),
                [0u8; 32],
            ),
            script_context: crate::context::CurrentScriptContext::input_lock_for_tests(
                current_lock_hash,
                input_locks,
            ),
            witnesses: scan_tx_level_witnesses(&[tx_witness]),
            otx_layouts: crate::layout::OtxLayouts::None,
        }
    }

    fn test_context_without_otx_layouts() -> CobuildContext {
        CobuildContext {
            tx: crate::syscalls::SyscallTxReader::from_cached_parts_for_tests(
                crate::syscalls::TxCounts::default(),
                crate::reader::cursor_from_slice(&empty_transaction()),
                [0u8; 32],
            ),
            script_context: crate::context::CurrentScriptContext::from_script_for_tests(
                CurrentScript::InputLock([0u8; 32]),
            ),
            witnesses: crate::witness::WitnessScan::with_capacity(0),
            otx_layouts: crate::layout::OtxLayouts::None,
        }
    }

    fn scan_tx_level_witnesses(witnesses: &[Vec<u8>]) -> crate::witness::WitnessScan {
        let mut scanner = CobuildWitnessScanner::with_capacity(witnesses.len());
        for witness in witnesses {
            scanner
                .push_witness(crate::reader::cursor_from_slice(witness))
                .unwrap();
        }
        scanner.finish(0, 0, 0, 0).unwrap().tx_level
    }

    fn empty_transaction() -> Vec<u8> {
        table_bytes(&[
            raw_transaction_bytes(),
            molecule_bytes(&[0u8; 32]),
            dynvec_bytes(&[]),
        ])
    }

    fn raw_transaction_bytes() -> Vec<u8> {
        table_bytes(&[
            0u32.to_le_bytes().to_vec(),
            dynvec_bytes(&[]),
            dynvec_bytes(&[]),
            dynvec_bytes(&[]),
            dynvec_bytes(&[]),
        ])
    }

    fn message_with_actions(actions: &[Vec<u8>]) -> Vec<u8> {
        table_bytes(&[dynvec_bytes(actions)])
    }

    fn action_bytes(script_role: u8, script_hash: [u8; 32], data: &[u8]) -> Vec<u8> {
        table_bytes(&[
            [0x33u8; 32].to_vec(),
            vec![script_role],
            script_hash.to_vec(),
            molecule_bytes(data),
        ])
    }

    fn sighash_all_witness_bytes(seal: &[u8], message: &[u8]) -> Vec<u8> {
        let item = table_bytes(&[molecule_bytes(seal), message.to_vec()]);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0xff00_0001u32.to_le_bytes());
        bytes.extend_from_slice(&item);
        bytes
    }

    fn dynvec_bytes(items: &[Vec<u8>]) -> Vec<u8> {
        if items.is_empty() {
            return 4u32.to_le_bytes().to_vec();
        }
        let header_size = 4 + items.len() * 4;
        let total_size = header_size + items.iter().map(Vec::len).sum::<usize>();
        let mut bytes = Vec::with_capacity(total_size);
        bytes.extend_from_slice(&(total_size as u32).to_le_bytes());
        let mut offset = header_size;
        for item in items {
            bytes.extend_from_slice(&(offset as u32).to_le_bytes());
            offset += item.len();
        }
        for item in items {
            bytes.extend_from_slice(item);
        }
        bytes
    }

    fn molecule_bytes(raw: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(4 + raw.len());
        bytes.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        bytes.extend_from_slice(raw);
        bytes
    }

    fn table_bytes(fields: &[Vec<u8>]) -> Vec<u8> {
        let header_size = 4 + fields.len() * 4;
        let total_size = header_size + fields.iter().map(Vec::len).sum::<usize>();
        let mut bytes = Vec::with_capacity(total_size);
        bytes.extend_from_slice(&(total_size as u32).to_le_bytes());
        let mut offset = header_size;
        for field in fields {
            bytes.extend_from_slice(&(offset as u32).to_le_bytes());
            offset += field.len();
        }
        for field in fields {
            bytes.extend_from_slice(field);
        }
        bytes
    }

    #[test]
    fn lock_related_tx_action_preserves_origin_and_action_cursor() {
        let action = test_action(ScriptRole::InputLock, [0x44; 32], &[0x99]);
        let related = related_tx_action(2, action.clone());

        assert!(matches!(
            related.origin,
            ActionOrigin::TxLevel { witness_index: 2 }
        ));
        assert_eq!(
            crate::reader::cursor_bytes(&related.action.data).unwrap(),
            vec![0x99]
        );
    }

    #[test]
    fn lock_related_otx_action_preserves_origin_layout_and_action_cursor() {
        let message_bytes = [4u8, 0, 0, 0];
        let otx = test_otx(&message_bytes, 1, 2);
        let action = test_action(ScriptRole::InputLock, [0x44; 32], &[0x88]);

        let related = related_otx_action(3, &otx, action);

        match related.origin {
            ActionOrigin::Otx {
                witness_index,
                otx_index,
                layout,
            } => {
                assert_eq!(witness_index, 7);
                assert_eq!(otx_index, 3);
                assert_eq!(layout.base_inputs.start, 1);
                assert_eq!(layout.append_inputs.start, 2);
            }
            ActionOrigin::TxLevel { .. } => panic!("expected OTX action origin"),
        }
        assert_eq!(
            crate::reader::cursor_bytes(&related.action.data).unwrap(),
            vec![0x88]
        );
    }

    #[test]
    fn lock_actions_for_message_returns_all_matching_actions() {
        let lock_hash = [0x44; 32];
        let other_hash = [0x55; 32];
        let message = message_with_actions(&[
            action_bytes(0, lock_hash, &[0x10]),
            action_bytes(0, lock_hash, &[0x20]),
            action_bytes(0, other_hash, &[0x30]),
        ]);

        let actions =
            lock_actions_for_message(&crate::reader::cursor_from_slice(&message), lock_hash)
                .unwrap();

        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].index, 0);
        assert_eq!(actions[1].index, 1);
    }

    #[test]
    fn append_segment_input_requires_segment_signature() {
        let lock_hash = [0x44; 32];
        let other_hash = [0x55; 32];
        let otx = test_otx(&message_with_actions(&[]), 0, 1);
        let context =
            lock_context_with_otx_entries(lock_hash, vec![other_hash, lock_hash], vec![otx]);
        let builder = LockPlanBuilder {
            context: &context,
            lock_script_hash: lock_hash,
            required_signatures: Vec::new(),
            related_actions: Vec::new(),
            tx_level_action_cache: None,
        };
        let OtxLayouts::Complete(layout) = &context.otx_layouts else {
            panic!("expected complete OTX layouts");
        };

        assert_eq!(
            builder
                .required_append_segment_indices(&layout.otx_entries[0])
                .unwrap(),
            vec![0]
        );
    }

    #[test]
    fn type_actions_for_message_returns_all_matching_type_actions() {
        let type_hash = [0x66; 32];
        let message = message_with_actions(&[
            action_bytes(1, type_hash, &[0x10]),
            action_bytes(1, type_hash, &[0x20]),
            action_bytes(2, type_hash, &[0x30]),
            action_bytes(0, type_hash, &[0x40]),
        ]);

        let actions =
            type_actions_for_message(&crate::reader::cursor_from_slice(&message), type_hash)
                .unwrap();

        assert_eq!(actions.len(), 3);
        assert_eq!(actions[0].script_role, ScriptRole::InputType);
        assert_eq!(actions[1].script_role, ScriptRole::InputType);
        assert_eq!(actions[2].script_role, ScriptRole::OutputType);
    }

    #[test]
    fn otx_actions_returns_related_actions_from_requested_otx() {
        let lock_hash = [0x44; 32];
        let other_lock_hash = [0x55; 32];
        let first_message = message_with_actions(&[action_bytes(0, lock_hash, &[0x10])]);
        let second_message = message_with_actions(&[
            action_bytes(0, lock_hash, &[0x20]),
            action_bytes(0, other_lock_hash, &[0x30]),
        ]);
        let context = lock_context_with_otx_entries(
            lock_hash,
            vec![lock_hash, other_lock_hash],
            vec![
                test_otx(&first_message, 0, 1),
                test_otx(&second_message, 1, 2),
            ],
        );

        let actions = context.otx_actions(1).unwrap();

        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].action.index, 0);
        assert_eq!(actions[0].action.script_role, ScriptRole::InputLock);
        assert_eq!(
            actions[0].action_ref(),
            crate::plan::ActionRef::Otx {
                witness_index: 7,
                otx_index: 1,
                action_index: 0,
            }
        );
        assert_eq!(
            crate::reader::cursor_bytes(&actions[0].action.data).unwrap(),
            vec![0x20]
        );
        assert_eq!(actions[1].action.index, 1);
        assert_eq!(actions[1].action.script_role, ScriptRole::InputLock);
        assert_eq!(
            crate::reader::cursor_bytes(&actions[1].action.data).unwrap(),
            vec![0x30]
        );
    }

    #[test]
    fn otx_actions_rejects_missing_or_out_of_range_otx() {
        assert_eq!(
            test_context_without_otx_layouts().otx_actions(0).err(),
            Some(CoreError::InvalidOtxLayout)
        );
        assert_eq!(
            test_context_with_otx_entries(Vec::new())
                .otx_actions(0)
                .err(),
            Some(CoreError::InvalidOtxLayout)
        );
    }

    #[test]
    fn otx_actions_rejects_unknown_targets() {
        let lock_hash = [0x44; 32];
        let missing_lock_hash = [0x55; 32];
        let message = message_with_actions(&[action_bytes(0, missing_lock_hash, &[0x10])]);
        let context = lock_context_with_otx_entries(
            lock_hash,
            vec![lock_hash],
            vec![test_otx(&message, 0, 1)],
        );

        assert_eq!(
            context.otx_actions(0).err(),
            Some(CoreError::InvalidMessageTarget)
        );
    }

    #[test]
    fn otx_actions_return_all_actions_with_otx_origins() {
        let lock_hash = [0x44; 32];
        let other_lock_hash = [0x55; 32];
        let message = message_with_actions(&[
            action_bytes(0, lock_hash, &[0x10]),
            action_bytes(0, other_lock_hash, &[0x20]),
        ]);
        let context = lock_context_with_otx_entries(
            lock_hash,
            vec![lock_hash, other_lock_hash],
            vec![test_otx(&message, 0, 1)],
        );

        let actions = context.otx_actions(0).unwrap();

        assert_eq!(actions.len(), 2);
        assert_eq!(
            actions[0].action_ref(),
            crate::plan::ActionRef::Otx {
                witness_index: 7,
                otx_index: 0,
                action_index: 0,
            }
        );
        assert_eq!(
            actions[1].action_ref(),
            crate::plan::ActionRef::Otx {
                witness_index: 7,
                otx_index: 0,
                action_index: 1,
            }
        );
    }

    #[test]
    fn tx_level_actions_return_unique_message_actions_with_tx_origins() {
        let lock_hash = [0x44; 32];
        let other_lock_hash = [0x55; 32];
        let message = message_with_actions(&[
            action_bytes(0, lock_hash, &[0x10]),
            action_bytes(0, other_lock_hash, &[0x20]),
        ]);
        let context =
            lock_context_with_tx_message(lock_hash, vec![lock_hash, other_lock_hash], &message);

        let actions = context.tx_level_actions().unwrap().unwrap();

        assert_eq!(actions.len(), 2);
        assert_eq!(
            actions[0].action_ref(),
            crate::plan::ActionRef::TxLevel {
                witness_index: 0,
                action_index: 0,
            }
        );
        assert_eq!(
            actions[1].action_ref(),
            crate::plan::ActionRef::TxLevel {
                witness_index: 0,
                action_index: 1,
            }
        );
    }

    #[test]
    fn tx_level_actions_rejects_unknown_targets() {
        let lock_hash = [0x44; 32];
        let missing_lock_hash = [0x55; 32];
        let message = message_with_actions(&[action_bytes(0, missing_lock_hash, &[0x10])]);
        let context = lock_context_with_tx_message(lock_hash, vec![lock_hash], &message);

        assert_eq!(
            context.tx_level_actions().err(),
            Some(CoreError::InvalidMessageTarget)
        );
    }

    #[test]
    fn all_actions_return_tx_level_actions_then_otx_actions() {
        let lock_hash = [0x44; 32];
        let tx_message = message_with_actions(&[action_bytes(0, lock_hash, &[0x10])]);
        let first_otx_message = message_with_actions(&[action_bytes(0, lock_hash, &[0x20])]);
        let second_otx_message = message_with_actions(&[action_bytes(0, lock_hash, &[0x30])]);
        let context = lock_context_with_tx_message_and_otx_entries(
            lock_hash,
            vec![lock_hash],
            &tx_message,
            vec![
                test_otx(&first_otx_message, 0, 1),
                test_otx(&second_otx_message, 1, 2),
            ],
        );

        let actions = context.all_actions().unwrap();

        assert_eq!(actions.len(), 3);
        assert_eq!(
            actions[0].action_ref(),
            crate::plan::ActionRef::TxLevel {
                witness_index: 0,
                action_index: 0,
            }
        );
        assert_eq!(
            actions[1].action_ref(),
            crate::plan::ActionRef::Otx {
                witness_index: 7,
                otx_index: 0,
                action_index: 0,
            }
        );
        assert_eq!(
            actions[2].action_ref(),
            crate::plan::ActionRef::Otx {
                witness_index: 7,
                otx_index: 1,
                action_index: 0,
            }
        );
    }

    #[test]
    fn all_actions_rejects_unknown_otx_targets() {
        let lock_hash = [0x44; 32];
        let missing_lock_hash = [0x55; 32];
        let tx_message = message_with_actions(&[action_bytes(0, lock_hash, &[0x10])]);
        let otx_message = message_with_actions(&[action_bytes(0, missing_lock_hash, &[0x20])]);
        let context = lock_context_with_tx_message_and_otx_entries(
            lock_hash,
            vec![lock_hash],
            &tx_message,
            vec![test_otx(&otx_message, 0, 1)],
        );

        assert_eq!(
            context.all_actions().err(),
            Some(CoreError::InvalidMessageTarget)
        );
    }

    #[test]
    fn find_action_returns_tx_level_action_by_ref() {
        let lock_hash = [0x44; 32];
        let tx_message = message_with_actions(&[
            action_bytes(0, lock_hash, &[0x10]),
            action_bytes(0, lock_hash, &[0x20]),
        ]);
        let context = lock_context_with_tx_message(lock_hash, vec![lock_hash], &tx_message);

        let action = context
            .find_action(crate::plan::ActionRef::TxLevel {
                witness_index: 0,
                action_index: 1,
            })
            .unwrap();

        assert_eq!(
            action.action_ref(),
            crate::plan::ActionRef::TxLevel {
                witness_index: 0,
                action_index: 1,
            }
        );
        assert_eq!(
            crate::reader::cursor_bytes(&action.action.data).unwrap(),
            vec![0x20]
        );
    }

    #[test]
    fn find_action_does_not_validate_action_target() {
        let lock_hash = [0x44; 32];
        let missing_lock_hash = [0x55; 32];
        let tx_message = message_with_actions(&[
            action_bytes(0, lock_hash, &[0x10]),
            action_bytes(0, missing_lock_hash, &[0x20]),
        ]);
        let context = lock_context_with_tx_message(lock_hash, vec![lock_hash], &tx_message);

        assert_eq!(
            context.tx_level_actions().err(),
            Some(CoreError::InvalidMessageTarget)
        );

        let action = context
            .find_action(crate::plan::ActionRef::TxLevel {
                witness_index: 0,
                action_index: 1,
            })
            .unwrap();

        assert_eq!(action.action.script_hash, missing_lock_hash);
        assert_eq!(
            crate::reader::cursor_bytes(&action.action.data).unwrap(),
            vec![0x20]
        );
    }

    #[test]
    fn find_action_returns_otx_action_by_ref() {
        let lock_hash = [0x44; 32];
        let other_lock_hash = [0x55; 32];
        let first_message = message_with_actions(&[action_bytes(0, lock_hash, &[0x10])]);
        let second_message = message_with_actions(&[
            action_bytes(0, other_lock_hash, &[0x20]),
            action_bytes(0, lock_hash, &[0x30]),
        ]);
        let context = lock_context_with_otx_entries(
            lock_hash,
            vec![lock_hash, other_lock_hash],
            vec![
                test_otx(&first_message, 0, 1),
                test_otx(&second_message, 1, 2),
            ],
        );

        let action = context
            .find_action(crate::plan::ActionRef::Otx {
                witness_index: 7,
                otx_index: 1,
                action_index: 0,
            })
            .unwrap();

        assert_eq!(
            action.action_ref(),
            crate::plan::ActionRef::Otx {
                witness_index: 7,
                otx_index: 1,
                action_index: 0,
            }
        );
        assert_eq!(
            crate::reader::cursor_bytes(&action.action.data).unwrap(),
            vec![0x20]
        );
    }

    #[test]
    fn find_action_rejects_mismatched_otx_witness_index() {
        let lock_hash = [0x44; 32];
        let message = message_with_actions(&[action_bytes(0, lock_hash, &[0x10])]);
        let context = lock_context_with_otx_entries(
            lock_hash,
            vec![lock_hash],
            vec![test_otx(&message, 0, 1)],
        );

        assert_eq!(
            context
                .find_action(crate::plan::ActionRef::Otx {
                    witness_index: 6,
                    otx_index: 0,
                    action_index: 0,
                })
                .err(),
            Some(CoreError::ActionNotFound)
        );
    }

    #[test]
    fn find_action_rejects_missing_otx_entry_as_action_not_found() {
        let lock_hash = [0x44; 32];
        let context = lock_context_with_otx_entries(lock_hash, vec![lock_hash], vec![]);

        assert_eq!(
            context
                .find_action(crate::plan::ActionRef::Otx {
                    witness_index: 7,
                    otx_index: 0,
                    action_index: 0,
                })
                .err(),
            Some(CoreError::ActionNotFound)
        );
    }

    #[test]
    fn find_action_rejects_otx_ref_without_otx_layout_as_action_not_found() {
        let lock_hash = [0x44; 32];
        let tx_message = message_with_actions(&[action_bytes(0, lock_hash, &[0x10])]);
        let context = lock_context_with_tx_message(lock_hash, vec![lock_hash], &tx_message);

        assert_eq!(
            context
                .find_action(crate::plan::ActionRef::Otx {
                    witness_index: 7,
                    otx_index: 0,
                    action_index: 0,
                })
                .err(),
            Some(CoreError::ActionNotFound)
        );
    }

    #[test]
    fn action_hash_binds_action_ref_and_action_content() {
        let lock_hash = [0x44; 32];
        let tx_message = message_with_actions(&[action_bytes(0, lock_hash, &[0x10])]);
        let context = lock_context_with_tx_message(lock_hash, vec![lock_hash], &tx_message);
        let action = context
            .find_action(crate::plan::ActionRef::TxLevel {
                witness_index: 0,
                action_index: 0,
            })
            .unwrap();

        let original = action.action_hash().unwrap();
        let same = crate::plan::action_hash(action.action_ref(), &action.action).unwrap();
        let different_ref = crate::plan::action_hash(
            crate::plan::ActionRef::TxLevel {
                witness_index: 1,
                action_index: 0,
            },
            &action.action,
        )
        .unwrap();
        let different_action = crate::plan::action_hash(
            action.action_ref(),
            &test_action(ScriptRole::InputLock, lock_hash, &[0x11]),
        )
        .unwrap();

        assert_eq!(original, same);
        assert_ne!(original, different_ref);
        assert_ne!(original, different_action);
    }

    #[test]
    fn otx_scope_is_target_only_when_type_only_matches_by_action() {
        let relation = crate::plan::OtxTypeRelation {
            input_type_in_base: false,
            input_type_in_append: false,
            output_type_in_base: false,
            output_type_in_base_covered: false,
            output_type_in_append: false,
        };

        assert_eq!(
            type_action_otx_scope(relation),
            TypeActionOtxScope::TargetOnly
        );
    }

    #[test]
    fn otx_scope_is_in_scope_for_each_type_relation_presence() {
        for relation in [
            type_relation(true, false, false, false, false),
            type_relation(false, true, false, false, false),
            type_relation(false, false, true, false, false),
            type_relation(false, false, true, true, false),
            type_relation(false, false, false, false, true),
        ] {
            assert_eq!(
                type_action_otx_scope(relation),
                TypeActionOtxScope::InOtxScope(relation)
            );
        }
    }

    #[test]
    fn otx_scope_preserves_uncovered_base_output_relation() {
        let relation = type_relation(false, false, true, false, false);

        assert_eq!(
            type_action_otx_scope(relation),
            TypeActionOtxScope::InOtxScope(relation)
        );
        assert_eq!(
            type_action_otx_scope(relation).in_otx_scope(),
            Some(relation)
        );
    }

    #[test]
    fn missing_lock_group_coverage_is_reachable_after_otx_signature_planning() {
        let lock_hash = [0x44; 32];
        let context = lock_context_with_input_range(
            lock_hash,
            vec![lock_hash, [0x55; 32], lock_hash],
            crate::layout::Range { start: 0, count: 2 },
        );
        let builder = LockPlanBuilder {
            context: &context,
            lock_script_hash: lock_hash,
            required_signatures: vec![SigningRequirement {
                origin: SignatureOrigin::OtxBase,
                carrier_witness_index: 0,
                seal: Vec::new(),
                signing_message_hash: [0u8; 32],
            }],
            related_actions: Vec::new(),
            tx_level_action_cache: None,
        };

        assert_eq!(
            builder.ensure_otx_lock_group_coverage(),
            Err(CoreError::MissingLockGroupCoverage)
        );
    }

    #[test]
    fn other_lock_inputs_outside_otx_range_do_not_require_current_tx_level_coverage() {
        let lock_hash = [0x44; 32];
        let context = lock_context_with_input_range(
            lock_hash,
            vec![lock_hash, [0x55; 32], [0x55; 32]],
            crate::layout::Range { start: 0, count: 1 },
        );
        let builder = LockPlanBuilder {
            context: &context,
            lock_script_hash: lock_hash,
            required_signatures: vec![SigningRequirement {
                origin: SignatureOrigin::OtxBase,
                carrier_witness_index: 0,
                seal: Vec::new(),
                signing_message_hash: [0u8; 32],
            }],
            related_actions: Vec::new(),
            tx_level_action_cache: None,
        };

        assert_eq!(builder.ensure_otx_lock_group_coverage(), Ok(()));
    }

    fn type_relation(
        input_type_in_base: bool,
        input_type_in_append: bool,
        output_type_in_base: bool,
        output_type_in_base_covered: bool,
        output_type_in_append: bool,
    ) -> crate::plan::OtxTypeRelation {
        crate::plan::OtxTypeRelation {
            input_type_in_base,
            input_type_in_append,
            output_type_in_base,
            output_type_in_base_covered,
            output_type_in_append,
        }
    }
}

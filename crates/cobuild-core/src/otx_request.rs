use alloc::vec::Vec;

use crate::{
    context::LockScriptQuery,
    error::CoreError,
    hash::{otx_append_hash, otx_base_hash},
    layout::{OtxLayoutScan, Range},
    signature::{SignatureOrigin, SignatureRequest},
    source::SigningDataSource,
};

impl LockScriptQuery<'_> {
    pub(crate) fn collect_otx_signatures<S: SigningDataSource>(
        &self,
        source: &S,
    ) -> Result<Vec<SignatureRequest>, CoreError> {
        let mut requests = Vec::new();
        let layout = match &self.context.layout_scan {
            OtxLayoutScan::None => return Ok(requests),
            OtxLayoutScan::Complete(layout) => layout,
            OtxLayoutScan::Invalid { anchor, error } => {
                return self.invalid_otx_layout_signatures(anchor.as_ref(), error.clone());
            }
        };

        for otx in &layout.otx_data {
            let base_relevant = self.script_in_range(otx.layout.base_inputs);
            let append_relevant = self.script_in_range(otx.layout.append_inputs);
            if !base_relevant && !append_relevant {
                continue;
            }

            self.validate_message_targets(&otx.witness.message)?;
            let base_hash = otx_base_hash(&otx.witness, &otx.layout, source)?;
            if base_relevant {
                requests.push(SignatureRequest {
                    script_hash: self.script_hash,
                    carrier_witness_index: otx.layout.witness_index,
                    origin: SignatureOrigin::OtxBase,
                    seal: self.unique_otx_base_seal(&otx.witness.seals)?,
                    signing_message_hash: base_hash,
                });
            }
            if append_relevant {
                requests.push(SignatureRequest {
                    script_hash: self.script_hash,
                    carrier_witness_index: otx.layout.witness_index,
                    origin: SignatureOrigin::OtxAppend,
                    seal: self.unique_otx_append_seal(&otx.witness.seals)?,
                    signing_message_hash: otx_append_hash(
                        &otx.witness,
                        &otx.layout,
                        source,
                        base_hash,
                    )?,
                });
            }
        }

        Ok(requests)
    }

    fn invalid_otx_layout_signatures(
        &self,
        anchor: Option<&crate::view::OtxStartView>,
        error: CoreError,
    ) -> Result<Vec<SignatureRequest>, CoreError> {
        let Some(anchor) = anchor else {
            return Ok(Vec::new());
        };
        let relevant = self
            .context
            .script_hashes
            .input_locks
            .iter()
            .skip(anchor.start_input_cell)
            .any(|hash| *hash == self.script_hash);
        if relevant {
            Err(error)
        } else {
            Ok(Vec::new())
        }
    }

    fn script_in_range(&self, range: Range) -> bool {
        self.context
            .script_hashes
            .input_locks
            .iter()
            .skip(range.start)
            .take(range.count)
            .any(|hash| *hash == self.script_hash)
    }

    pub(crate) fn otx_covers_input(&self, input_index: usize) -> bool {
        let OtxLayoutScan::Complete(layout) = &self.context.layout_scan else {
            return false;
        };
        layout.otxs.iter().any(|otx| {
            range_contains(otx.base_inputs, input_index)
                || range_contains(otx.append_inputs, input_index)
        })
    }
}

fn range_contains(range: Range, index: usize) -> bool {
    index >= range.start && index < range.start.saturating_add(range.count)
}

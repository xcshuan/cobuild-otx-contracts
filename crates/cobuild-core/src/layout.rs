use alloc::vec::Vec;

use crate::{
    error::CoreError,
    protocol::{AppendPermissions, SealScope},
    view::{OtxStartView, OtxView, WitnessLayoutView},
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LayoutTx {
    pub witnesses: Vec<Vec<u8>>,
    pub input_count: usize,
    pub output_count: usize,
    pub cell_dep_count: usize,
    pub header_dep_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Range {
    pub start: usize,
    pub count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OtxLayout {
    pub witness_index: usize,
    pub base_inputs: Range,
    pub append_inputs: Range,
    pub base_outputs: Range,
    pub append_outputs: Range,
    pub base_cell_deps: Range,
    pub append_cell_deps: Range,
    pub base_header_deps: Range,
    pub append_header_deps: Range,
}

#[derive(Clone)]
pub struct OtxLayoutEntry {
    pub layout: OtxLayout,
    pub witness: OtxView,
}

#[derive(Clone)]
pub struct BuiltLayout {
    pub otxs: Vec<OtxLayout>,
    pub otx_entries: Vec<OtxLayoutEntry>,
}

#[derive(Clone)]
pub enum OtxLayoutScan {
    None,
    Complete(BuiltLayout),
    Invalid {
        anchor: Option<OtxStartView>,
        error: CoreError,
    },
}

pub(crate) struct OtxLayoutCollector {
    witness_count: usize,
    start: Option<(usize, OtxStartView)>,
    last_otx_or_start: Option<usize>,
    otx_entries: Vec<(usize, OtxView)>,
    invalid: Option<OtxLayoutScan>,
}

pub fn build_layout(tx: &LayoutTx) -> Result<BuiltLayout, CoreError> {
    match scan_layout(tx) {
        OtxLayoutScan::None => Ok(empty_layout()),
        OtxLayoutScan::Complete(layout) => Ok(layout),
        OtxLayoutScan::Invalid { error, .. } => Err(error),
    }
}

pub fn build_layout_from_witnesses(
    tx: &LayoutTx,
    input_count: usize,
    output_count: usize,
    cell_dep_count: usize,
    header_dep_count: usize,
) -> Result<BuiltLayout, CoreError> {
    match scan_layout_from_witnesses(
        tx,
        input_count,
        output_count,
        cell_dep_count,
        header_dep_count,
    ) {
        OtxLayoutScan::None => Ok(empty_layout()),
        OtxLayoutScan::Complete(layout) => Ok(layout),
        OtxLayoutScan::Invalid { error, .. } => Err(error),
    }
}

pub fn scan_layout(tx: &LayoutTx) -> OtxLayoutScan {
    scan_layout_from_witnesses(
        tx,
        tx.input_count,
        tx.output_count,
        tx.cell_dep_count,
        tx.header_dep_count,
    )
}

pub(crate) fn scan_layout_from_witnesses(
    tx: &LayoutTx,
    input_count: usize,
    output_count: usize,
    cell_dep_count: usize,
    header_dep_count: usize,
) -> OtxLayoutScan {
    let mut collector = OtxLayoutCollector::new();
    for index in 0..tx.witnesses.len() {
        let witness = match witness_bytes_from_layout_tx(tx, index) {
            Ok(witness) => witness,
            Err(error) => {
                return OtxLayoutScan::Invalid {
                    anchor: None,
                    error,
                };
            }
        };
        collector.push_witness(&witness);
        if collector.has_invalid() {
            return collector.finish(input_count, output_count, cell_dep_count, header_dep_count);
        }
    }

    collector.finish(input_count, output_count, cell_dep_count, header_dep_count)
}

impl OtxLayoutCollector {
    pub(crate) fn new() -> Self {
        Self {
            witness_count: 0,
            start: None,
            last_otx_or_start: None,
            otx_entries: Vec::new(),
            invalid: None,
        }
    }

    pub(crate) fn push_witness(&mut self, witness: &[u8]) {
        let index = self.witness_count;
        self.witness_count += 1;

        if self.invalid.is_some() {
            return;
        }

        if witness.is_empty() {
            return;
        }

        let view = match WitnessLayoutView::from_slice(witness) {
            Ok(view) => view,
            Err(error) => {
                if has_otx_witness_id(witness) {
                    self.set_invalid(self.start.as_ref().map(|(_, data)| data.clone()), error);
                }
                return;
            }
        };

        let otx_start = match view.otx_start() {
            Ok(data) => data,
            Err(error) => {
                self.set_invalid(None, error);
                return;
            }
        };
        if let Some(data) = otx_start {
            if self.start.is_some() {
                self.set_invalid(
                    self.start.as_ref().map(|(_, data)| data.clone()),
                    CoreError::InvalidOtxLayout,
                );
                return;
            }
            self.start = Some((index, data));
            self.last_otx_or_start = Some(index);
            return;
        }

        let otx = match view.otx() {
            Ok(data) => data,
            Err(error) => {
                self.set_invalid(None, error);
                return;
            }
        };
        let Some(data) = otx else {
            return;
        };

        let Some(last_index) = self.last_otx_or_start else {
            self.set_invalid(None, CoreError::InvalidOtxLayout);
            return;
        };
        if last_index + 1 != index {
            self.set_invalid(None, CoreError::InvalidOtxLayout);
            return;
        }

        self.last_otx_or_start = Some(index);
        self.otx_entries.push((index, data));
    }

    pub(crate) fn finish(
        self,
        input_count: usize,
        output_count: usize,
        cell_dep_count: usize,
        header_dep_count: usize,
    ) -> OtxLayoutScan {
        if let Some(invalid) = self.invalid {
            return invalid;
        }

        let Some((start_witness_index, start_data)) = self.start else {
            return OtxLayoutScan::None;
        };
        if self.last_otx_or_start == Some(start_witness_index) {
            return invalid_layout(Some(&start_data), CoreError::InvalidOtxLayout);
        }

        let mut next_input = start_data.start_input_cell;
        let mut next_output = start_data.start_output_cell;
        let mut next_cell_dep = start_data.start_cell_deps;
        let mut next_header_dep = start_data.start_header_deps;
        let mut otxs = Vec::new();
        let mut otx_entries = Vec::new();

        for (witness_index, otx_witness) in self.otx_entries {
            if let Err(error) = validate_otx_view(&otx_witness) {
                return invalid_layout(Some(&start_data), error);
            }

            let base_inputs = match take_range(&mut next_input, otx_witness.base_input_cells) {
                Ok(range) => range,
                Err(error) => return invalid_layout(Some(&start_data), error),
            };
            let append_inputs = match take_range(&mut next_input, otx_witness.append_input_cells) {
                Ok(range) => range,
                Err(error) => return invalid_layout(Some(&start_data), error),
            };
            let base_outputs = match take_range(&mut next_output, otx_witness.base_output_cells) {
                Ok(range) => range,
                Err(error) => return invalid_layout(Some(&start_data), error),
            };
            let append_outputs = match take_range(&mut next_output, otx_witness.append_output_cells)
            {
                Ok(range) => range,
                Err(error) => return invalid_layout(Some(&start_data), error),
            };
            let base_cell_deps = match take_range(&mut next_cell_dep, otx_witness.base_cell_deps) {
                Ok(range) => range,
                Err(error) => return invalid_layout(Some(&start_data), error),
            };
            let append_cell_deps =
                match take_range(&mut next_cell_dep, otx_witness.append_cell_deps) {
                    Ok(range) => range,
                    Err(error) => return invalid_layout(Some(&start_data), error),
                };
            let base_header_deps =
                match take_range(&mut next_header_dep, otx_witness.base_header_deps) {
                    Ok(range) => range,
                    Err(error) => return invalid_layout(Some(&start_data), error),
                };
            let append_header_deps =
                match take_range(&mut next_header_dep, otx_witness.append_header_deps) {
                    Ok(range) => range,
                    Err(error) => return invalid_layout(Some(&start_data), error),
                };

            let layout = OtxLayout {
                witness_index,
                base_inputs,
                append_inputs,
                base_outputs,
                append_outputs,
                base_cell_deps,
                append_cell_deps,
                base_header_deps,
                append_header_deps,
            };
            otxs.push(layout.clone());
            otx_entries.push(OtxLayoutEntry {
                layout,
                witness: otx_witness,
            });
        }

        if otxs.is_empty() {
            return invalid_layout(Some(&start_data), CoreError::InvalidOtxLayout);
        }
        if let Err(error) = ensure_within(next_input, input_count) {
            return invalid_layout(Some(&start_data), error);
        }
        if let Err(error) = ensure_within(next_output, output_count) {
            return invalid_layout(Some(&start_data), error);
        }
        if let Err(error) = ensure_within(next_cell_dep, cell_dep_count) {
            return invalid_layout(Some(&start_data), error);
        }
        if let Err(error) = ensure_within(next_header_dep, header_dep_count) {
            return invalid_layout(Some(&start_data), error);
        }

        OtxLayoutScan::Complete(BuiltLayout { otxs, otx_entries })
    }

    fn has_invalid(&self) -> bool {
        self.invalid.is_some()
    }

    fn set_invalid(&mut self, anchor: Option<OtxStartView>, error: CoreError) {
        self.invalid = Some(OtxLayoutScan::Invalid { anchor, error });
    }
}

fn witness_bytes_from_layout_tx(tx: &LayoutTx, index: usize) -> Result<Vec<u8>, CoreError> {
    tx.witnesses
        .get(index)
        .cloned()
        .ok_or(CoreError::MissingHashInput)
}

fn invalid_layout(anchor: Option<&OtxStartView>, error: CoreError) -> OtxLayoutScan {
    OtxLayoutScan::Invalid {
        anchor: anchor.cloned(),
        error,
    }
}

fn empty_layout() -> BuiltLayout {
    BuiltLayout {
        otxs: Vec::new(),
        otx_entries: Vec::new(),
    }
}

fn take_range(start: &mut usize, count: usize) -> Result<Range, CoreError> {
    let range = Range {
        start: *start,
        count,
    };
    *start = start
        .checked_add(count)
        .ok_or(CoreError::InvalidOtxLayout)?;
    Ok(range)
}

fn ensure_within(value: usize, max: usize) -> Result<(), CoreError> {
    if value <= max {
        Ok(())
    } else {
        Err(CoreError::InvalidOtxLayout)
    }
}

fn validate_otx_view(data: &OtxView) -> Result<(), CoreError> {
    if data.base_input_cells == 0 {
        return Err(CoreError::InvalidOtxLayout);
    }
    let append_permissions = AppendPermissions::try_from(data.append_permissions)?;
    append_permissions.require_allowed(0, data.append_input_cells)?;
    append_permissions.require_allowed(1, data.append_output_cells)?;
    append_permissions.require_allowed(2, data.append_cell_deps)?;
    append_permissions.require_allowed(3, data.append_header_deps)?;
    data.base_input_masks.validate(data.base_input_cells * 2)?;
    data.base_output_masks
        .validate(data.base_output_cells * 4)?;
    data.base_cell_dep_masks.validate(data.base_cell_deps)?;
    data.base_header_dep_masks.validate(data.base_header_deps)?;
    for seal in &data.seals {
        SealScope::try_from(seal.scope)?;
    }
    Ok(())
}

fn has_otx_witness_id(witness: &[u8]) -> bool {
    if witness.len() < 4 {
        return false;
    }
    let item_id = u32::from_le_bytes([witness[0], witness[1], witness[2], witness[3]]);
    matches!(item_id, 0xff00_0003 | 0xff00_0004)
}

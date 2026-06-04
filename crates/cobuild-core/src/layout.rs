use alloc::vec::Vec;

use crate::{
    error::CoreError,
    protocol::AppendPermissions,
    reader::{cursor_bytes_with_error, cursor_from_slice},
    source::ClassifiedCursor,
    view::{MaskView, OtxStartView, OtxView, WitnessLayoutView},
};

#[derive(Clone, Debug, Eq, PartialEq)]
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
pub struct OtxLayoutData {
    pub layout: OtxLayout,
    pub witness: OtxView,
}

#[derive(Clone)]
pub struct BuiltLayout {
    pub otxs: Vec<OtxLayout>,
    pub otx_data: Vec<OtxLayoutData>,
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
    otx_data: Vec<(usize, OtxView)>,
    first_segment_stop: Option<LayoutSegmentStop>,
    invalid: Option<OtxLayoutScan>,
}

enum LayoutSegmentStop {
    Break,
    Invalid(CoreError),
}

pub trait WitnessCursorSource {
    fn witness_count(&self) -> usize;
    fn witness_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>;
}

impl WitnessCursorSource for LayoutTx {
    fn witness_count(&self) -> usize {
        self.witnesses.len()
    }

    fn witness_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        self.witnesses
            .get(index)
            .map(|witness| ClassifiedCursor::hash_input(cursor_from_slice(witness)))
            .ok_or(CoreError::MissingHashInput)
    }
}

pub fn build_layout(tx: &LayoutTx) -> Result<BuiltLayout, CoreError> {
    match scan_layout(tx) {
        OtxLayoutScan::None => Ok(empty_layout()),
        OtxLayoutScan::Complete(layout) => Ok(layout),
        OtxLayoutScan::Invalid { error, .. } => Err(error),
    }
}

pub fn build_layout_from_witnesses<S: WitnessCursorSource>(
    source: &S,
    input_count: usize,
    output_count: usize,
    cell_dep_count: usize,
    header_dep_count: usize,
) -> Result<BuiltLayout, CoreError> {
    match scan_layout_from_witnesses(
        source,
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

pub(crate) fn scan_layout_from_witnesses<S: WitnessCursorSource>(
    source: &S,
    input_count: usize,
    output_count: usize,
    cell_dep_count: usize,
    header_dep_count: usize,
) -> OtxLayoutScan {
    let witness_count = source.witness_count();
    if witness_count == 0 {
        return OtxLayoutScan::None;
    }

    let mut collector = OtxLayoutCollector::new();
    for index in 0..witness_count {
        let witness = match witness_bytes(source, index) {
            Ok(witness) => witness,
            Err(error) => return invalid_layout(None, error),
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
            otx_data: Vec::new(),
            first_segment_stop: None,
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
            self.mark_segment_break();
            return;
        }

        let view = match WitnessLayoutView::from_slice(witness) {
            Ok(view) => view,
            Err(error) => {
                self.mark_segment_invalid(error);
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
            self.mark_segment_break();
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
        self.otx_data.push((index, data));
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
        let mut otx_data = Vec::new();

        for (witness_index, data) in self.otx_data {
            if let Err(error) = validate_otx_data(&data) {
                return invalid_layout(Some(&start_data), error);
            }

            let base_inputs = match take_range(&mut next_input, data.base_input_cells) {
                Ok(range) => range,
                Err(error) => return invalid_layout(Some(&start_data), error),
            };
            let append_inputs = match take_range(&mut next_input, data.append_input_cells) {
                Ok(range) => range,
                Err(error) => return invalid_layout(Some(&start_data), error),
            };
            let base_outputs = match take_range(&mut next_output, data.base_output_cells) {
                Ok(range) => range,
                Err(error) => return invalid_layout(Some(&start_data), error),
            };
            let append_outputs = match take_range(&mut next_output, data.append_output_cells) {
                Ok(range) => range,
                Err(error) => return invalid_layout(Some(&start_data), error),
            };
            let base_cell_deps = match take_range(&mut next_cell_dep, data.base_cell_deps) {
                Ok(range) => range,
                Err(error) => return invalid_layout(Some(&start_data), error),
            };
            let append_cell_deps = match take_range(&mut next_cell_dep, data.append_cell_deps) {
                Ok(range) => range,
                Err(error) => return invalid_layout(Some(&start_data), error),
            };
            let base_header_deps = match take_range(&mut next_header_dep, data.base_header_deps) {
                Ok(range) => range,
                Err(error) => return invalid_layout(Some(&start_data), error),
            };
            let append_header_deps = match take_range(&mut next_header_dep, data.append_header_deps)
            {
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
            otx_data.push(OtxLayoutData {
                layout,
                witness: data,
            });
        }

        if let Some(LayoutSegmentStop::Invalid(error)) = self.first_segment_stop {
            return invalid_layout(Some(&start_data), error);
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

        OtxLayoutScan::Complete(BuiltLayout { otxs, otx_data })
    }

    fn has_invalid(&self) -> bool {
        self.invalid.is_some()
    }

    fn mark_segment_break(&mut self) {
        if self.start.is_some() && !self.otx_data.is_empty() && self.first_segment_stop.is_none() {
            self.first_segment_stop = Some(LayoutSegmentStop::Break);
        }
    }

    fn mark_segment_invalid(&mut self, error: CoreError) {
        if self.start.is_some() && !self.otx_data.is_empty() && self.first_segment_stop.is_none() {
            self.first_segment_stop = Some(LayoutSegmentStop::Invalid(error));
        }
    }

    fn set_invalid(&mut self, anchor: Option<OtxStartView>, error: CoreError) {
        self.invalid = Some(OtxLayoutScan::Invalid { anchor, error });
    }
}

fn witness_bytes<S: WitnessCursorSource>(source: &S, index: usize) -> Result<Vec<u8>, CoreError> {
    let classified = source.witness_cursor(index)?;
    cursor_bytes_with_error(&classified.cursor, classified.read_error())
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
        otx_data: Vec::new(),
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

fn validate_otx_data(data: &OtxView) -> Result<(), CoreError> {
    if data.base_input_cells == 0 {
        return Err(CoreError::InvalidOtxLayout);
    }
    let append_permissions = AppendPermissions::try_from(data.append_permissions)?;
    append_permissions.require_allowed(0, data.append_input_cells)?;
    append_permissions.require_allowed(1, data.append_output_cells)?;
    append_permissions.require_allowed(2, data.append_cell_deps)?;
    append_permissions.require_allowed(3, data.append_header_deps)?;
    validate_mask(&data.base_input_masks, data.base_input_cells * 2)?;
    validate_mask(&data.base_output_masks, data.base_output_cells * 4)?;
    validate_mask(&data.base_cell_dep_masks, data.base_cell_deps)?;
    validate_mask(&data.base_header_dep_masks, data.base_header_deps)?;
    Ok(())
}

fn validate_mask(mask: &MaskView, bit_count: usize) -> Result<(), CoreError> {
    let expected_len = bit_count.div_ceil(8);
    if mask.len() != expected_len {
        return Err(CoreError::InvalidOtxLayout);
    }
    if bit_count == 0 {
        return Ok(());
    }
    let used_bits = bit_count % 8;
    if used_bits == 0 {
        return Ok(());
    }
    for index in bit_count..(expected_len * 8) {
        if mask.bit(index)? {
            return Err(CoreError::InvalidOtxLayout);
        }
    }
    Ok(())
}

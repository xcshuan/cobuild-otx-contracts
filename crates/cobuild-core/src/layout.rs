use alloc::vec::Vec;

use crate::{
    error::CoreError,
    protocol::{AppendPermissions, SealScope},
    view::{CobuildWitnessLayoutView, OtxStartView, OtxView},
};

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
    pub otx_entries: Vec<OtxLayoutEntry>,
}

#[derive(Clone)]
pub enum OtxLayouts {
    None,
    Complete(BuiltLayout),
}

pub(crate) struct OtxLayoutCollector {
    start: Option<(usize, OtxStartView)>,
    last_layout_witness_index: Option<usize>,
    otx_entries: Vec<(usize, OtxView)>,
}

enum LayoutWitnessView {
    Ignore,
    OtxStart(OtxStartView),
    Otx(OtxView),
}

impl OtxLayoutCollector {
    pub(crate) fn new() -> Self {
        Self {
            start: None,
            last_layout_witness_index: None,
            otx_entries: Vec::new(),
        }
    }

    pub(crate) fn record_cobuild_layout(
        &mut self,
        index: usize,
        view: &CobuildWitnessLayoutView,
    ) -> Result<(), CoreError> {
        match Self::layout_witness_view(view)? {
            LayoutWitnessView::Ignore => Ok(()),
            LayoutWitnessView::OtxStart(view) => self.push_otx_start(index, view),
            LayoutWitnessView::Otx(view) => self.push_otx(index, view),
        }
    }

    pub(crate) fn finish(
        self,
        input_count: usize,
        output_count: usize,
        cell_dep_count: usize,
        header_dep_count: usize,
    ) -> Result<OtxLayouts, CoreError> {
        let Some((start_witness_index, start_data)) = self.start else {
            return Ok(OtxLayouts::None);
        };
        if self.last_layout_witness_index == Some(start_witness_index) {
            return Err(CoreError::InvalidOtxLayout);
        }

        let mut ranges = LayoutRangeCursor::from_start(&start_data);
        let mut otx_entries = Vec::new();

        for (witness_index, otx_view) in self.otx_entries {
            validate_otx_view(&otx_view)?;
            let layout = ranges.take_layout(witness_index, &otx_view)?;
            otx_entries.push(OtxLayoutEntry {
                layout,
                witness: otx_view,
            });
        }

        if otx_entries.is_empty() {
            return Err(CoreError::InvalidOtxLayout);
        }
        ranges.ensure_within(input_count, output_count, cell_dep_count, header_dep_count)?;

        Ok(OtxLayouts::Complete(BuiltLayout { otx_entries }))
    }

    fn layout_witness_view(
        view: &CobuildWitnessLayoutView,
    ) -> Result<LayoutWitnessView, CoreError> {
        match view.otx_start() {
            Ok(Some(data)) => return Ok(LayoutWitnessView::OtxStart(data)),
            Ok(None) => {}
            Err(error) => return Err(error),
        }

        match view.otx() {
            Ok(Some(data)) => Ok(LayoutWitnessView::Otx(data)),
            Ok(None) => Ok(LayoutWitnessView::Ignore),
            Err(error) => Err(error),
        }
    }

    fn push_otx_start(&mut self, index: usize, view: OtxStartView) -> Result<(), CoreError> {
        if self.start.is_some() {
            return Err(CoreError::InvalidOtxLayout);
        }

        self.start = Some((index, view));
        self.last_layout_witness_index = Some(index);
        Ok(())
    }

    fn push_otx(&mut self, index: usize, view: OtxView) -> Result<(), CoreError> {
        let Some(last_index) = self.last_layout_witness_index else {
            return Err(CoreError::InvalidOtxLayout);
        };
        if last_index + 1 != index {
            return Err(CoreError::InvalidOtxLayout);
        }

        self.last_layout_witness_index = Some(index);
        self.otx_entries.push((index, view));
        Ok(())
    }
}

struct LayoutRangeCursor {
    next_input: usize,
    next_output: usize,
    next_cell_dep: usize,
    next_header_dep: usize,
}

impl LayoutRangeCursor {
    fn from_start(start: &OtxStartView) -> Self {
        Self {
            next_input: start.start_input_cell,
            next_output: start.start_output_cell,
            next_cell_dep: start.start_cell_deps,
            next_header_dep: start.start_header_deps,
        }
    }

    fn take_layout(
        &mut self,
        witness_index: usize,
        otx_view: &OtxView,
    ) -> Result<OtxLayout, CoreError> {
        Ok(OtxLayout {
            witness_index,
            base_inputs: Self::take_range(&mut self.next_input, otx_view.base_input_cells)?,
            append_inputs: Self::take_range(&mut self.next_input, otx_view.append_input_cells)?,
            base_outputs: Self::take_range(&mut self.next_output, otx_view.base_output_cells)?,
            append_outputs: Self::take_range(&mut self.next_output, otx_view.append_output_cells)?,
            base_cell_deps: Self::take_range(&mut self.next_cell_dep, otx_view.base_cell_deps)?,
            append_cell_deps: Self::take_range(&mut self.next_cell_dep, otx_view.append_cell_deps)?,
            base_header_deps: Self::take_range(
                &mut self.next_header_dep,
                otx_view.base_header_deps,
            )?,
            append_header_deps: Self::take_range(
                &mut self.next_header_dep,
                otx_view.append_header_deps,
            )?,
        })
    }

    fn take_range(next: &mut usize, count: usize) -> Result<Range, CoreError> {
        let range = Range {
            start: *next,
            count,
        };
        *next = next.checked_add(count).ok_or(CoreError::InvalidOtxLayout)?;
        Ok(range)
    }

    fn ensure_within(
        &self,
        input_count: usize,
        output_count: usize,
        cell_dep_count: usize,
        header_dep_count: usize,
    ) -> Result<(), CoreError> {
        ensure_within(self.next_input, input_count)?;
        ensure_within(self.next_output, output_count)?;
        ensure_within(self.next_cell_dep, cell_dep_count)?;
        ensure_within(self.next_header_dep, header_dep_count)?;
        Ok(())
    }
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

#[cfg(test)]
mod tests;

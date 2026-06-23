use alloc::vec::Vec;

use crate::{
    error::CoreError,
    protocol::{
        AppendPermissions, SegmentFlags, APPEND_PERMISSION_CELL_DEPS_BIT,
        APPEND_PERMISSION_HEADER_DEPS_BIT, APPEND_PERMISSION_INPUTS_BIT,
        APPEND_PERMISSION_OUTPUTS_BIT,
    },
    view::{CobuildWitnessLayoutView, OtxStartView, OtxView},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Range {
    pub start: usize,
    pub count: usize,
}

impl Range {
    pub fn end(&self) -> usize {
        self.start
            .checked_add(self.count)
            .expect("valid cobuild layout range")
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn contains(&self, index: usize) -> bool {
        let Some(end) = self.start.checked_add(self.count) else {
            return false;
        };
        index >= self.start && index < end
    }

    pub fn local_index(&self, index: usize) -> Option<usize> {
        if self.contains(index) {
            Some(index - self.start)
        } else {
            None
        }
    }

    pub fn indexes(&self) -> IndexRange {
        IndexRange {
            start: self.start,
            end: self.end(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexRange {
    pub start: usize,
    pub end: usize,
}

impl IntoIterator for IndexRange {
    type Item = usize;
    type IntoIter = core::ops::Range<usize>;

    fn into_iter(self) -> Self::IntoIter {
        self.start..self.end
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OtxAppendSegmentLayout {
    pub segment_index: usize,
    pub flags: crate::protocol::SegmentFlags,
    pub inputs: Range,
    pub outputs: Range,
    pub cell_deps: Range,
    pub header_deps: Range,
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
    pub append_segments: Vec<OtxAppendSegmentLayout>,
}

#[derive(Clone)]
pub struct OtxLayoutEntry {
    pub layout: OtxLayout,
    pub witness: OtxView,
}

#[derive(Clone)]
pub struct BuiltLayout {
    pub input_range: Range,
    pub output_range: Range,
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

        Ok(OtxLayouts::Complete(BuiltLayout {
            input_range: Range {
                start: start_data.start_input_cell,
                count: ranges.next_input - start_data.start_input_cell,
            },
            output_range: Range {
                start: start_data.start_output_cell,
                count: ranges.next_output - start_data.start_output_cell,
            },
            otx_entries,
        }))
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
        let base_inputs = Self::take_range(&mut self.next_input, otx_view.base_input_cells)?;
        let base_outputs = Self::take_range(&mut self.next_output, otx_view.base_output_cells)?;
        let base_cell_deps = Self::take_range(&mut self.next_cell_dep, otx_view.base_cell_deps)?;
        let base_header_deps =
            Self::take_range(&mut self.next_header_dep, otx_view.base_header_deps)?;

        let append_input_start = self.next_input;
        let append_output_start = self.next_output;
        let append_cell_dep_start = self.next_cell_dep;
        let append_header_dep_start = self.next_header_dep;
        let mut append_segments = Vec::with_capacity(otx_view.append_segments.len());
        for (segment_index, segment) in otx_view.append_segments.iter().enumerate() {
            append_segments.push(OtxAppendSegmentLayout {
                segment_index,
                flags: SegmentFlags::try_from(segment.segment_flags)?,
                inputs: Self::take_range(&mut self.next_input, segment.input_cells)?,
                outputs: Self::take_range(&mut self.next_output, segment.output_cells)?,
                cell_deps: Self::take_range(&mut self.next_cell_dep, segment.cell_deps)?,
                header_deps: Self::take_range(&mut self.next_header_dep, segment.header_deps)?,
            });
        }

        Ok(OtxLayout {
            witness_index,
            base_inputs,
            append_inputs: Range {
                start: append_input_start,
                count: self.next_input - append_input_start,
            },
            base_outputs,
            append_outputs: Range {
                start: append_output_start,
                count: self.next_output - append_output_start,
            },
            base_cell_deps,
            append_cell_deps: Range {
                start: append_cell_dep_start,
                count: self.next_cell_dep - append_cell_dep_start,
            },
            base_header_deps,
            append_header_deps: Range {
                start: append_header_dep_start,
                count: self.next_header_dep - append_header_dep_start,
            },
            append_segments,
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
    for (index, segment) in data.append_segments.iter().enumerate() {
        let flags = SegmentFlags::try_from(segment.segment_flags)?;
        if index + 1 != data.append_segments.len() && !flags.allow_more_segments_after() {
            return Err(CoreError::InvalidOtxLayout);
        }
        append_permissions.require_allowed(APPEND_PERMISSION_INPUTS_BIT, segment.input_cells)?;
        append_permissions.require_allowed(APPEND_PERMISSION_OUTPUTS_BIT, segment.output_cells)?;
        append_permissions.require_allowed(APPEND_PERMISSION_CELL_DEPS_BIT, segment.cell_deps)?;
        append_permissions
            .require_allowed(APPEND_PERMISSION_HEADER_DEPS_BIT, segment.header_deps)?;
    }
    data.base_input_masks.validate(data.base_input_cells * 2)?;
    data.base_output_masks
        .validate(data.base_output_cells * 4)?;
    data.base_cell_dep_masks.validate(data.base_cell_deps)?;
    data.base_header_dep_masks.validate(data.base_header_deps)?;
    Ok(())
}

#[cfg(test)]
mod tests;

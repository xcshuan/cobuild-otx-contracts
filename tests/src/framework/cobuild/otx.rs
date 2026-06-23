use ckb_testtool::ckb_types::prelude::{Builder, Entity};
use cobuild_core::protocol::{
    APPEND_PERMISSION_CELL_DEPS_BIT, APPEND_PERMISSION_HEADER_DEPS_BIT,
    APPEND_PERMISSION_INPUTS_BIT, APPEND_PERMISSION_OUTPUTS_BIT,
};
use cobuild_types::entity::core::{
    ActionVec, LockSeal, LockSealVec, Message as CobuildMessage, Otx, OtxAppendSegment,
    OtxAppendSegmentVec,
};

#[derive(Clone, Debug)]
pub struct OtxBuilder {
    message: CobuildMessage,
    append_permissions: u8,
    base_input_cells: u32,
    base_input_masks: Vec<u8>,
    base_output_cells: u32,
    base_output_masks: Vec<u8>,
    base_cell_deps: u32,
    base_cell_dep_masks: Vec<u8>,
    base_header_deps: u32,
    base_header_dep_masks: Vec<u8>,
    append_segments: Vec<OtxAppendSegmentSpec>,
    base_seals: Vec<LockSeal>,
    auto_allow_more_after: bool,
}

#[derive(Clone, Debug, Default)]
pub struct OtxAppendSegmentSpec {
    pub flags: u8,
    pub input_cells: u32,
    pub output_cells: u32,
    pub cell_deps: u32,
    pub header_deps: u32,
    pub seals: Vec<LockSeal>,
}

#[derive(Clone, Debug)]
pub struct BuiltOtxSpec {
    pub otx: Otx,
    pub base_input_cells: u32,
    pub base_output_cells: u32,
    pub base_cell_deps: u32,
    pub base_header_deps: u32,
    pub append_input_cells: u32,
    pub append_output_cells: u32,
    pub append_cell_deps: u32,
    pub append_header_deps: u32,
    pub append_segments: Vec<BuiltOtxAppendSegmentSpec>,
}

#[derive(Clone, Debug)]
pub struct BuiltOtxAppendSegmentSpec {
    pub flags: u8,
    pub input_cells: u32,
    pub output_cells: u32,
    pub cell_deps: u32,
    pub header_deps: u32,
}

#[derive(Clone, Debug)]
pub struct RawOtxBuilder {
    inner: OtxBuilder,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BaseInputMaskField {
    Since,
    PreviousOutput,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BaseOutputMaskField {
    Capacity,
    Lock,
    Type,
    Data,
}

#[derive(Clone, Debug)]
pub struct BaseInputMaskDsl {
    input_count: usize,
    masks: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct BaseOutputMaskDsl {
    output_count: usize,
    masks: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct ItemMaskDsl {
    count: usize,
    name: &'static str,
    masks: Vec<u8>,
}

impl OtxBuilder {
    pub fn new() -> Self {
        Self {
            message: empty_message(),
            append_permissions: 0,
            base_input_cells: 0,
            base_input_masks: Vec::new(),
            base_output_cells: 0,
            base_output_masks: Vec::new(),
            base_cell_deps: 0,
            base_cell_dep_masks: Vec::new(),
            base_header_deps: 0,
            base_header_dep_masks: Vec::new(),
            append_segments: Vec::new(),
            base_seals: Vec::new(),
            auto_allow_more_after: true,
        }
    }

    pub fn message(mut self, message: CobuildMessage) -> Self {
        self.message = message;
        self
    }

    pub(crate) fn base_input_mask_bytes(mut self, masks: Vec<u8>) -> Self {
        self.base_input_masks = masks;
        self
    }

    pub(crate) fn base_output_mask_bytes(mut self, masks: Vec<u8>) -> Self {
        self.base_output_masks = masks;
        self
    }

    pub(crate) fn base_cell_dep_mask_bytes(mut self, masks: Vec<u8>) -> Self {
        self.base_cell_dep_masks = masks;
        self
    }

    pub(crate) fn base_header_dep_mask_bytes(mut self, masks: Vec<u8>) -> Self {
        self.base_header_dep_masks = masks;
        self
    }

    pub fn base_input_cells(mut self, count: u32) -> Self {
        self.base_input_cells = count;
        self.base_input_masks = zero_masks(count as usize * 2);
        self
    }

    pub fn base_output_cells(mut self, count: u32) -> Self {
        self.base_output_cells = count;
        self.base_output_masks = if count == 0 {
            Vec::new()
        } else {
            full_base_output_masks(count as usize)
        };
        self
    }

    pub fn base_cell_deps(mut self, count: u32) -> Self {
        self.base_cell_deps = count;
        self.base_cell_dep_masks = zero_masks(count as usize);
        self
    }

    pub fn base_header_deps(mut self, count: u32) -> Self {
        self.base_header_deps = count;
        self.base_header_dep_masks = zero_masks(count as usize);
        self
    }

    pub fn append_segment(
        mut self,
        flags: u8,
        input_cells: u32,
        output_cells: u32,
        cell_deps: u32,
        header_deps: u32,
        seals: Vec<LockSeal>,
    ) -> Self {
        self.append_segments.push(OtxAppendSegmentSpec {
            flags,
            input_cells,
            output_cells,
            cell_deps,
            header_deps,
            seals,
        });
        self
    }

    pub fn base_seals(mut self, seals: Vec<LockSeal>) -> Self {
        self.base_seals = seals;
        self
    }

    pub fn allow_append_inputs(mut self) -> Self {
        self.append_permissions |= 1 << APPEND_PERMISSION_INPUTS_BIT;
        self
    }

    pub fn allow_append_outputs(mut self) -> Self {
        self.append_permissions |= 1 << APPEND_PERMISSION_OUTPUTS_BIT;
        self
    }

    pub fn allow_append_cell_deps(mut self) -> Self {
        self.append_permissions |= 1 << APPEND_PERMISSION_CELL_DEPS_BIT;
        self
    }

    pub fn allow_append_header_deps(mut self) -> Self {
        self.append_permissions |= 1 << APPEND_PERMISSION_HEADER_DEPS_BIT;
        self
    }

    pub fn cover_base_input_since(mut self, local_input: usize) -> Self {
        set_mask_bit(&mut self.base_input_masks, local_input * 2, true);
        self
    }

    pub fn uncover_base_input_since(mut self, local_input: usize) -> Self {
        set_mask_bit(&mut self.base_input_masks, local_input * 2, false);
        self
    }

    pub fn cover_base_input_previous_output(mut self, local_input: usize) -> Self {
        set_mask_bit(&mut self.base_input_masks, local_input * 2 + 1, true);
        self
    }

    pub fn uncover_base_input_previous_output(mut self, local_input: usize) -> Self {
        set_mask_bit(&mut self.base_input_masks, local_input * 2 + 1, false);
        self
    }

    pub fn cover_base_output_capacity(mut self, local_output: usize) -> Self {
        set_mask_bit(&mut self.base_output_masks, local_output * 4, true);
        self
    }

    pub fn uncover_base_output_capacity(mut self, local_output: usize) -> Self {
        set_mask_bit(&mut self.base_output_masks, local_output * 4, false);
        self
    }

    pub fn cover_base_output_lock(mut self, local_output: usize) -> Self {
        set_mask_bit(&mut self.base_output_masks, local_output * 4 + 1, true);
        self
    }

    pub fn uncover_base_output_lock(mut self, local_output: usize) -> Self {
        set_mask_bit(&mut self.base_output_masks, local_output * 4 + 1, false);
        self
    }

    pub fn cover_base_output_type(mut self, local_output: usize) -> Self {
        set_mask_bit(&mut self.base_output_masks, local_output * 4 + 2, true);
        self
    }

    pub fn uncover_base_output_type(mut self, local_output: usize) -> Self {
        set_mask_bit(&mut self.base_output_masks, local_output * 4 + 2, false);
        self
    }

    pub fn cover_base_output_data(mut self, local_output: usize) -> Self {
        set_mask_bit(&mut self.base_output_masks, local_output * 4 + 3, true);
        self
    }

    pub fn uncover_base_output_data(mut self, local_output: usize) -> Self {
        set_mask_bit(&mut self.base_output_masks, local_output * 4 + 3, false);
        self
    }

    pub fn build(self) -> Otx {
        self.build_with_layout().otx
    }

    pub fn build_with_layout(self) -> BuiltOtxSpec {
        let base_input_cells = self.base_input_cells;
        let base_output_cells = self.base_output_cells;
        let base_cell_deps = self.base_cell_deps;
        let base_header_deps = self.base_header_deps;
        let append_input_cells = self
            .append_segments
            .iter()
            .map(|segment| segment.input_cells)
            .sum();
        let append_output_cells = self
            .append_segments
            .iter()
            .map(|segment| segment.output_cells)
            .sum();
        let append_cell_deps = self
            .append_segments
            .iter()
            .map(|segment| segment.cell_deps)
            .sum();
        let append_header_deps = self
            .append_segments
            .iter()
            .map(|segment| segment.header_deps)
            .sum();
        let append_segment_count = self.append_segments.len();
        let append_segments: Vec<_> = self
            .append_segments
            .into_iter()
            .enumerate()
            .map(|(index, segment)| {
                let flags = if self.auto_allow_more_after && index + 1 < append_segment_count {
                    segment.flags | 0x01
                } else {
                    segment.flags
                };
                let built = OtxAppendSegment::new_builder()
                    .segment_flags(flags)
                    .input_cells(segment.input_cells.to_le_bytes())
                    .output_cells(segment.output_cells.to_le_bytes())
                    .cell_deps(segment.cell_deps.to_le_bytes())
                    .header_deps(segment.header_deps.to_le_bytes())
                    .seals(LockSealVec::new_builder().extend(segment.seals).build())
                    .build();
                (
                    BuiltOtxAppendSegmentSpec {
                        flags,
                        input_cells: segment.input_cells,
                        output_cells: segment.output_cells,
                        cell_deps: segment.cell_deps,
                        header_deps: segment.header_deps,
                    },
                    built,
                )
            })
            .collect();
        let append_segment_facts = append_segments
            .iter()
            .map(|(facts, _)| facts.clone())
            .collect();
        let append_segment_entities = append_segments.into_iter().map(|(_, segment)| segment);
        let otx = Otx::new_builder()
            .message(self.message)
            .append_permissions(self.append_permissions)
            .base_input_cells(self.base_input_cells.to_le_bytes())
            .base_input_masks(self.base_input_masks)
            .base_output_cells(self.base_output_cells.to_le_bytes())
            .base_output_masks(self.base_output_masks)
            .base_cell_deps(self.base_cell_deps.to_le_bytes())
            .base_cell_dep_masks(self.base_cell_dep_masks)
            .base_header_deps(self.base_header_deps.to_le_bytes())
            .base_header_dep_masks(self.base_header_dep_masks)
            .append_segments(
                OtxAppendSegmentVec::new_builder()
                    .extend(append_segment_entities)
                    .build(),
            )
            .base_seals(LockSealVec::new_builder().extend(self.base_seals).build())
            .build();
        BuiltOtxSpec {
            otx,
            base_input_cells,
            base_output_cells,
            base_cell_deps,
            base_header_deps,
            append_input_cells,
            append_output_cells,
            append_cell_deps,
            append_header_deps,
            append_segments: append_segment_facts,
        }
    }
}

impl Default for OtxBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl RawOtxBuilder {
    pub fn new() -> Self {
        let mut inner = OtxBuilder::new();
        inner.auto_allow_more_after = false;
        Self { inner }
    }

    pub fn message(mut self, message: CobuildMessage) -> Self {
        self.inner = self.inner.message(message);
        self
    }

    pub fn append_permissions(mut self, value: u8) -> Self {
        self.inner.append_permissions = value;
        self
    }

    pub fn base_input_cells(mut self, value: u32) -> Self {
        self.inner.base_input_cells = value;
        self
    }

    pub fn base_input_masks(mut self, masks: Vec<u8>) -> Self {
        self.inner.base_input_masks = masks;
        self
    }

    pub fn base_output_cells(mut self, value: u32) -> Self {
        self.inner.base_output_cells = value;
        self
    }

    pub fn base_output_masks(mut self, masks: Vec<u8>) -> Self {
        self.inner.base_output_masks = masks;
        self
    }

    pub fn base_cell_deps(mut self, value: u32) -> Self {
        self.inner.base_cell_deps = value;
        self
    }

    pub fn base_cell_dep_masks(mut self, masks: Vec<u8>) -> Self {
        self.inner.base_cell_dep_masks = masks;
        self
    }

    pub fn base_header_deps(mut self, value: u32) -> Self {
        self.inner.base_header_deps = value;
        self
    }

    pub fn base_header_dep_masks(mut self, masks: Vec<u8>) -> Self {
        self.inner.base_header_dep_masks = masks;
        self
    }

    pub fn base_seals(mut self, seals: Vec<LockSeal>) -> Self {
        self.inner = self.inner.base_seals(seals);
        self
    }

    pub fn append_segment(
        mut self,
        flags: u8,
        input_cells: u32,
        output_cells: u32,
        cell_deps: u32,
        header_deps: u32,
        seals: Vec<LockSeal>,
    ) -> Self {
        self.inner = self.inner.append_segment(
            flags,
            input_cells,
            output_cells,
            cell_deps,
            header_deps,
            seals,
        );
        self
    }

    pub fn allow_append_inputs(mut self) -> Self {
        self.inner = self.inner.allow_append_inputs();
        self
    }

    pub fn allow_append_outputs(mut self) -> Self {
        self.inner = self.inner.allow_append_outputs();
        self
    }

    pub fn allow_append_cell_deps(mut self) -> Self {
        self.inner = self.inner.allow_append_cell_deps();
        self
    }

    pub fn allow_append_header_deps(mut self) -> Self {
        self.inner = self.inner.allow_append_header_deps();
        self
    }

    pub fn build(self) -> Otx {
        self.inner.build()
    }

    pub fn build_with_layout(self) -> BuiltOtxSpec {
        self.inner.build_with_layout()
    }
}

impl Default for RawOtxBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub fn full_base_input_masks(input_count: usize) -> Vec<u8> {
    full_masks(input_count * 2, "input")
}

pub fn full_base_output_masks(output_count: usize) -> Vec<u8> {
    full_masks(output_count * 4, "output")
}

pub fn full_base_cell_dep_masks(cell_dep_count: usize) -> Vec<u8> {
    full_masks(cell_dep_count, "cell dep")
}

pub fn full_base_header_dep_masks(header_dep_count: usize) -> Vec<u8> {
    full_masks(header_dep_count, "header dep")
}

pub fn base_input_masks(input_count: usize, fields: &[(usize, BaseInputMaskField)]) -> Vec<u8> {
    let mut builder = base_input_mask(input_count);
    for &(local_input, field) in fields {
        builder = builder.cover_field(local_input, field);
    }
    builder.bytes()
}

pub fn base_output_masks(output_count: usize, fields: &[(usize, BaseOutputMaskField)]) -> Vec<u8> {
    let mut builder = base_output_mask(output_count);
    for &(local_output, field) in fields {
        builder = builder.cover_field(local_output, field);
    }
    builder.bytes()
}

pub fn base_cell_dep_masks(cell_dep_count: usize, covered_indexes: &[usize]) -> Vec<u8> {
    let mut builder = base_cell_dep_item_mask(cell_dep_count);
    for &index in covered_indexes {
        builder = builder.cover_item(index);
    }
    builder.bytes()
}

pub fn base_header_dep_masks(header_dep_count: usize, covered_indexes: &[usize]) -> Vec<u8> {
    let mut builder = base_header_dep_item_mask(header_dep_count);
    for &index in covered_indexes {
        builder = builder.cover_item(index);
    }
    builder.bytes()
}

pub fn base_input_mask(input_count: usize) -> BaseInputMaskDsl {
    BaseInputMaskDsl {
        input_count,
        masks: zero_masks(input_count * 2),
    }
}

pub fn base_output_mask(output_count: usize) -> BaseOutputMaskDsl {
    BaseOutputMaskDsl {
        output_count,
        masks: zero_masks(output_count * 4),
    }
}

pub fn base_cell_dep_item_mask(cell_dep_count: usize) -> ItemMaskDsl {
    item_mask(cell_dep_count, "cell dep")
}

pub fn base_header_dep_item_mask(header_dep_count: usize) -> ItemMaskDsl {
    item_mask(header_dep_count, "header dep")
}

impl BaseInputMaskDsl {
    pub fn cover_field(mut self, local_input: usize, field: BaseInputMaskField) -> Self {
        assert!(
            local_input < self.input_count,
            "input mask index {local_input} outside input count {}",
            self.input_count
        );
        let field_offset = match field {
            BaseInputMaskField::Since => 0,
            BaseInputMaskField::PreviousOutput => 1,
        };
        set_mask_bit(&mut self.masks, local_input * 2 + field_offset, true);
        self
    }

    pub fn bytes(self) -> Vec<u8> {
        self.masks
    }
}

impl BaseOutputMaskDsl {
    pub fn cover_field(mut self, local_output: usize, field: BaseOutputMaskField) -> Self {
        assert!(
            local_output < self.output_count,
            "output mask index {local_output} outside output count {}",
            self.output_count
        );
        let field_offset = match field {
            BaseOutputMaskField::Capacity => 0,
            BaseOutputMaskField::Lock => 1,
            BaseOutputMaskField::Type => 2,
            BaseOutputMaskField::Data => 3,
        };
        set_mask_bit(&mut self.masks, local_output * 4 + field_offset, true);
        self
    }

    pub fn bytes(self) -> Vec<u8> {
        self.masks
    }
}

impl ItemMaskDsl {
    pub fn cover_item(mut self, index: usize) -> Self {
        assert!(
            index < self.count,
            "{} mask index {index} outside {} count {}",
            self.name,
            self.name,
            self.count
        );
        set_mask_bit(&mut self.masks, index, true);
        self
    }

    pub fn bytes(self) -> Vec<u8> {
        self.masks
    }
}

fn full_masks(bits: usize, name: &str) -> Vec<u8> {
    let bytes = bits.div_ceil(8);
    let mut masks = vec![0xff; bytes];
    let extra_bits = bytes * 8 - bits;
    if extra_bits > 0 {
        let keep_bits = 8 - extra_bits;
        let last = masks
            .last_mut()
            .unwrap_or_else(|| panic!("non-empty {name} mask"));
        *last = (1u8 << keep_bits) - 1;
    }
    masks
}

fn item_mask(count: usize, name: &'static str) -> ItemMaskDsl {
    ItemMaskDsl {
        count,
        name,
        masks: zero_masks(count),
    }
}

fn zero_masks(bits: usize) -> Vec<u8> {
    vec![0; bits.div_ceil(8)]
}

fn set_mask_bit(masks: &mut Vec<u8>, bit: usize, covered: bool) {
    let byte = bit / 8;
    if masks.len() <= byte {
        masks.resize(byte + 1, 0);
    }
    let mask = 1u8 << (bit % 8);
    if covered {
        masks[byte] |= mask;
    } else {
        masks[byte] &= !mask;
    }
}

pub type OtxSpec = OtxBuilder;
pub type BuiltOtx = BuiltOtxSpec;

fn empty_message() -> CobuildMessage {
    CobuildMessage::new_builder()
        .actions(ActionVec::new_builder().build())
        .build()
}

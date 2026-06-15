use ckb_testtool::ckb_types::prelude::{Builder, Entity};
use cobuild_types::entity::core::{
    ActionVec, Message as CobuildMessage, Otx, SealPair, SealPairVec,
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
    append_input_cells: u32,
    append_output_cells: u32,
    append_cell_deps: u32,
    append_header_deps: u32,
    seals: Vec<SealPair>,
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
            append_input_cells: 0,
            append_output_cells: 0,
            append_cell_deps: 0,
            append_header_deps: 0,
            seals: Vec::new(),
        }
    }

    pub fn message(mut self, message: CobuildMessage) -> Self {
        self.message = message;
        self
    }

    pub fn append_permissions_raw(mut self, value: u8) -> Self {
        self.append_permissions = value;
        self
    }

    pub fn base_input_masks_raw(mut self, masks: Vec<u8>) -> Self {
        self.base_input_masks = masks;
        self
    }

    pub fn base_output_masks_raw(mut self, masks: Vec<u8>) -> Self {
        self.base_output_masks = masks;
        self
    }

    pub fn base_cell_dep_masks_raw(mut self, masks: Vec<u8>) -> Self {
        self.base_cell_dep_masks = masks;
        self
    }

    pub fn base_header_dep_masks_raw(mut self, masks: Vec<u8>) -> Self {
        self.base_header_dep_masks = masks;
        self
    }

    pub fn raw_base_input_cells(mut self, value: u32) -> Self {
        self.base_input_cells = value;
        self
    }

    pub fn raw_append_output_cells(mut self, value: u32) -> Self {
        self.append_output_cells = value;
        self
    }

    pub fn base_input_cells(mut self, count: u32) -> Self {
        self.base_input_cells = count;
        self.base_input_masks = zero_masks(count as usize * 2);
        self
    }

    pub fn append_output_cells(mut self, count: u32) -> Self {
        self.append_output_cells = count;
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

    pub fn append_input_cells(mut self, count: u32) -> Self {
        self.append_input_cells = count;
        self
    }

    pub fn append_cell_deps(mut self, count: u32) -> Self {
        self.append_cell_deps = count;
        self
    }

    pub fn append_header_deps(mut self, count: u32) -> Self {
        self.append_header_deps = count;
        self
    }

    pub fn seals(mut self, seals: Vec<SealPair>) -> Self {
        self.seals = seals;
        self
    }

    pub fn allow_append_inputs(mut self) -> Self {
        self.append_permissions |= 0b0001;
        self
    }

    pub fn allow_append_outputs(mut self) -> Self {
        self.append_permissions |= 0b0010;
        self
    }

    pub fn allow_append_cell_deps(mut self) -> Self {
        self.append_permissions |= 0b0100;
        self
    }

    pub fn allow_append_header_deps(mut self) -> Self {
        self.append_permissions |= 0b1000;
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
        let append_input_cells = self.append_input_cells;
        let append_output_cells = self.append_output_cells;
        let append_cell_deps = self.append_cell_deps;
        let append_header_deps = self.append_header_deps;
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
            .append_input_cells(self.append_input_cells.to_le_bytes())
            .append_output_cells(self.append_output_cells.to_le_bytes())
            .append_cell_deps(self.append_cell_deps.to_le_bytes())
            .append_header_deps(self.append_header_deps.to_le_bytes())
            .seals(SealPairVec::new_builder().extend(self.seals).build())
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
        }
    }
}

impl Default for OtxBuilder {
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
    let mut masks = zero_masks(input_count * 2);
    for &(local_input, field) in fields {
        assert!(
            local_input < input_count,
            "input mask index {local_input} outside input count {input_count}"
        );
        let field_offset = match field {
            BaseInputMaskField::Since => 0,
            BaseInputMaskField::PreviousOutput => 1,
        };
        set_mask_bit(&mut masks, local_input * 2 + field_offset, true);
    }
    masks
}

pub fn base_output_masks(output_count: usize, fields: &[(usize, BaseOutputMaskField)]) -> Vec<u8> {
    let mut masks = zero_masks(output_count * 4);
    for &(local_output, field) in fields {
        assert!(
            local_output < output_count,
            "output mask index {local_output} outside output count {output_count}"
        );
        let field_offset = match field {
            BaseOutputMaskField::Capacity => 0,
            BaseOutputMaskField::Lock => 1,
            BaseOutputMaskField::Type => 2,
            BaseOutputMaskField::Data => 3,
        };
        set_mask_bit(&mut masks, local_output * 4 + field_offset, true);
    }
    masks
}

pub fn base_cell_dep_masks(cell_dep_count: usize, covered_indexes: &[usize]) -> Vec<u8> {
    indexed_masks(cell_dep_count, covered_indexes, "cell dep")
}

pub fn base_header_dep_masks(header_dep_count: usize, covered_indexes: &[usize]) -> Vec<u8> {
    indexed_masks(header_dep_count, covered_indexes, "header dep")
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

fn indexed_masks(count: usize, covered_indexes: &[usize], name: &str) -> Vec<u8> {
    let mut masks = zero_masks(count);
    for &index in covered_indexes {
        assert!(
            index < count,
            "{name} mask index {index} outside {name} count {count}"
        );
        set_mask_bit(&mut masks, index, true);
    }
    masks
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

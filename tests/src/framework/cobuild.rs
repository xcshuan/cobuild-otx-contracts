use ckb_testtool::ckb_types::prelude::{Builder, Entity};
use cobuild_types::entity::core::{Action, ActionVec, Message as CobuildMessage, Otx, SealPairVec};

pub fn empty_message() -> CobuildMessage {
    CobuildMessage::new_builder()
        .actions(ActionVec::new_builder().build())
        .build()
}

pub fn seal_pair(
    script_hash: [u8; 32],
    scope: u8,
    seal: Vec<u8>,
) -> cobuild_types::entity::core::SealPair {
    cobuild_types::entity::core::SealPair::new_builder()
        .script_hash(script_hash)
        .scope(scope)
        .seal(seal)
        .build()
}

#[derive(Clone, Debug)]
pub struct CobuildMessageBuilder {
    script_hash: [u8; 32],
    script_role: u8,
    action_data: Vec<u8>,
}

impl CobuildMessageBuilder {
    pub fn new() -> Self {
        Self {
            script_hash: [0; 32],
            script_role: 1,
            action_data: Vec::new(),
        }
    }

    pub fn input_type_action(mut self, script_hash: [u8; 32]) -> Self {
        self.script_hash = script_hash;
        self.script_role = 1;
        self
    }

    pub fn action_data(mut self, action_data: Vec<u8>) -> Self {
        self.action_data = action_data;
        self
    }

    pub fn build(self) -> CobuildMessage {
        let action = Action::new_builder()
            .script_info_hash([0u8; 32])
            .script_role(self.script_role)
            .script_hash(self.script_hash)
            .data(self.action_data)
            .build();

        CobuildMessage::new_builder()
            .actions(ActionVec::new_builder().push(action).build())
            .build()
    }
}

impl Default for CobuildMessageBuilder {
    fn default() -> Self {
        Self::new()
    }
}

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
}

#[derive(Clone, Debug)]
pub struct BuiltOtx {
    pub otx: Otx,
    pub base_input_cells: u32,
    pub append_output_cells: u32,
}

impl OtxBuilder {
    pub fn new() -> Self {
        Self {
            message: CobuildMessageBuilder::new().build(),
            append_permissions: 0,
            base_input_cells: 1,
            base_input_masks: vec![0],
            base_output_cells: 0,
            base_output_masks: Vec::new(),
            base_cell_deps: 0,
            base_cell_dep_masks: Vec::new(),
            base_header_deps: 0,
            base_header_dep_masks: Vec::new(),
            append_input_cells: 0,
            append_output_cells: 1,
            append_cell_deps: 0,
            append_header_deps: 0,
        }
    }

    pub fn message(mut self, message: CobuildMessage) -> Self {
        self.message = message;
        self
    }

    pub fn base_input_cells(mut self, count: u32) -> Self {
        self.base_input_cells = count;
        self.base_input_masks = if count == 0 { Vec::new() } else { vec![0] };
        self
    }

    pub fn append_output_cells(mut self, count: u32) -> Self {
        self.append_output_cells = count;
        self
    }

    pub fn allow_append_outputs(mut self) -> Self {
        self.append_permissions |= 0b0010;
        self
    }

    pub fn build(self) -> Otx {
        self.build_with_layout().otx
    }

    pub fn build_with_layout(self) -> BuiltOtx {
        let base_input_cells = self.base_input_cells;
        let append_output_cells = self.append_output_cells;
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
            .seals(SealPairVec::new_builder().build())
            .build();
        BuiltOtx {
            otx,
            base_input_cells,
            append_output_cells,
        }
    }
}

impl Default for OtxBuilder {
    fn default() -> Self {
        Self::new()
    }
}

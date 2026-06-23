use alloc::vec::Vec;

use cobuild_types::lazy_reader::{
    core::{Action, LockSeal, Message, Otx, OtxAppendSegment, OtxStart},
    support::Cursor,
    witness::WitnessLayout as CobuildWitnessLayout,
};

use crate::{error::CoreError, protocol::ScriptRole, reader::cursor_bytes};

const BASE_INPUT_MASK_BITS_PER_CELL: usize = 2;
const BASE_INPUT_SINCE_MASK_OFFSET: usize = 0;
const BASE_INPUT_PREVIOUS_OUTPUT_MASK_OFFSET: usize = 1;

const BASE_OUTPUT_MASK_BITS_PER_CELL: usize = 4;
const BASE_OUTPUT_CAPACITY_MASK_OFFSET: usize = 0;
const BASE_OUTPUT_LOCK_MASK_OFFSET: usize = 1;
const BASE_OUTPUT_TYPE_MASK_OFFSET: usize = 2;
const BASE_OUTPUT_DATA_MASK_OFFSET: usize = 3;

pub struct CobuildWitnessLayoutView {
    #[allow(dead_code)]
    pub(crate) inner: CobuildWitnessLayout,
}

#[derive(Clone)]
pub enum SighashAllWitnessView {
    WithMessage { seal: Cursor, message: Cursor },
    SealOnly { seal: Cursor },
}

#[derive(Clone)]
pub struct OtxStartView {
    pub start_input_cell: usize,
    pub start_output_cell: usize,
    pub start_cell_deps: usize,
    pub start_header_deps: usize,
}

#[derive(Clone)]
pub struct MaskView {
    bytes: Vec<u8>,
}

#[derive(Clone)]
pub struct LockSealView {
    pub script_hash: [u8; 32],
    pub seal: Cursor,
}

#[derive(Clone)]
pub struct OtxAppendSegmentView {
    pub segment_flags: u8,
    pub input_cells: usize,
    pub output_cells: usize,
    pub cell_deps: usize,
    pub header_deps: usize,
    pub seals: Vec<LockSealView>,
}

#[derive(Clone)]
pub struct ActionView {
    pub index: usize,
    pub script_info_hash: [u8; 32],
    pub script_role: ScriptRole,
    pub script_hash: [u8; 32],
    pub data: Cursor,
}

#[derive(Clone)]
pub struct OtxView {
    pub message: Cursor,
    pub append_permissions: u8,
    pub base_input_cells: usize,
    pub base_input_masks: MaskView,
    pub base_output_cells: usize,
    pub base_output_masks: MaskView,
    pub base_cell_deps: usize,
    pub base_cell_dep_masks: MaskView,
    pub base_header_deps: usize,
    pub base_header_dep_masks: MaskView,
    pub append_segments: Vec<OtxAppendSegmentView>,
    pub base_seals: Vec<LockSealView>,
}

impl OtxView {
    pub fn includes_base_input_since(&self, local_index: usize) -> Result<bool, CoreError> {
        self.base_input_masks.get(base_input_mask_bit(
            local_index,
            BASE_INPUT_SINCE_MASK_OFFSET,
        ))
    }

    pub fn includes_base_input_previous_output(
        &self,
        local_index: usize,
    ) -> Result<bool, CoreError> {
        self.base_input_masks.get(base_input_mask_bit(
            local_index,
            BASE_INPUT_PREVIOUS_OUTPUT_MASK_OFFSET,
        ))
    }

    pub fn includes_base_output_capacity(&self, local_index: usize) -> Result<bool, CoreError> {
        self.base_output_masks.get(base_output_mask_bit(
            local_index,
            BASE_OUTPUT_CAPACITY_MASK_OFFSET,
        ))
    }

    pub fn includes_base_output_lock(&self, local_index: usize) -> Result<bool, CoreError> {
        self.base_output_masks.get(base_output_mask_bit(
            local_index,
            BASE_OUTPUT_LOCK_MASK_OFFSET,
        ))
    }

    pub fn includes_base_output_type(&self, local_index: usize) -> Result<bool, CoreError> {
        self.base_output_masks.get(base_output_mask_bit(
            local_index,
            BASE_OUTPUT_TYPE_MASK_OFFSET,
        ))
    }

    pub fn includes_base_output_data(&self, local_index: usize) -> Result<bool, CoreError> {
        self.base_output_masks.get(base_output_mask_bit(
            local_index,
            BASE_OUTPUT_DATA_MASK_OFFSET,
        ))
    }
}

#[derive(Clone)]
pub struct MessageView {
    cursor: Cursor,
}

impl MessageView {
    pub fn new(cursor: Cursor) -> Self {
        Self { cursor }
    }

    pub fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    pub fn actions(&self) -> Result<Vec<ActionView>, CoreError> {
        verify_message(&self.cursor)?;
        parse_actions_from_verified_message(&self.cursor)
    }

    pub fn action(&self, index: usize) -> Result<Option<ActionView>, CoreError> {
        verify_message(&self.cursor)?;
        parse_action_from_verified_message(&self.cursor, index)
    }

    pub(crate) fn actions_from_verified_message(&self) -> Result<Vec<ActionView>, CoreError> {
        parse_actions_from_verified_message(&self.cursor)
    }

    pub(crate) fn action_from_verified_message(
        &self,
        index: usize,
    ) -> Result<Option<ActionView>, CoreError> {
        parse_action_from_verified_message(&self.cursor, index)
    }

    pub fn actions_for(
        &self,
        role: ScriptRole,
        script_hash: [u8; 32],
    ) -> Result<Vec<ActionView>, CoreError> {
        Ok(self
            .actions()?
            .into_iter()
            .filter(|action| action.script_role == role && action.script_hash == script_hash)
            .collect())
    }

    pub fn unique_action_for(
        &self,
        role: ScriptRole,
        script_hash: [u8; 32],
    ) -> Result<Option<ActionView>, CoreError> {
        let mut matches = self.actions_for(role, script_hash)?;
        match matches.len() {
            0 => Ok(None),
            1 => Ok(matches.pop()),
            _ => Err(CoreError::DuplicateMatchingAction),
        }
    }
}

impl From<Cursor> for MessageView {
    fn from(cursor: Cursor) -> Self {
        Self::new(cursor)
    }
}

impl MaskView {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub fn get(&self, index: usize) -> Result<bool, CoreError> {
        let byte = *self
            .bytes
            .get(index / 8)
            .ok_or(CoreError::InvalidOtxLayout)?;
        Ok(byte & (1 << (index % 8)) != 0)
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn validate(&self, bit_count: usize) -> Result<(), CoreError> {
        let expected_len = bit_count.div_ceil(8);
        if self.len() != expected_len {
            return Err(CoreError::InvalidOtxLayout);
        }
        let used_bits = bit_count % 8;
        if used_bits == 0 {
            return Ok(());
        }

        let used_mask = (1u8 << used_bits) - 1;
        let last_byte = self.bytes[expected_len - 1];
        if last_byte & !used_mask != 0 {
            return Err(CoreError::InvalidOtxLayout);
        }

        Ok(())
    }
}

fn base_input_mask_bit(local_index: usize, field_offset: usize) -> usize {
    local_index * BASE_INPUT_MASK_BITS_PER_CELL + field_offset
}

fn base_output_mask_bit(local_index: usize, field_offset: usize) -> usize {
    local_index * BASE_OUTPUT_MASK_BITS_PER_CELL + field_offset
}

impl CobuildWitnessLayoutView {
    pub fn from_cursor(cursor: Cursor) -> Result<Self, CoreError> {
        let inner =
            CobuildWitnessLayout::try_from(cursor).map_err(|_| CoreError::MalformedCobuild)?;

        inner
            .verify(false)
            .map_err(|_| CoreError::InvalidOtxLayout)?;

        Ok(Self { inner })
    }

    pub fn sighash_all_cobuild_witness_layout(
        &self,
    ) -> Result<Option<SighashAllWitnessView>, CoreError> {
        match &self.inner {
            CobuildWitnessLayout::SighashAll(witness) => {
                let seal = witness.seal().map_err(|_| CoreError::MalformedCobuild)?;
                let message = witness.message().map_err(|_| CoreError::MalformedCobuild)?;
                Ok(Some(SighashAllWitnessView::WithMessage {
                    seal,
                    message: message.cursor,
                }))
            }
            CobuildWitnessLayout::SighashAllOnly(witness) => witness
                .seal()
                .map(|seal| Some(SighashAllWitnessView::SealOnly { seal }))
                .map_err(|_| CoreError::MalformedCobuild),
            _ => Ok(None),
        }
    }

    pub fn otx_start(&self) -> Result<Option<OtxStartView>, CoreError> {
        match &self.inner {
            CobuildWitnessLayout::OtxStart(start) => otx_start_view(start).map(Some),
            _ => Ok(None),
        }
    }

    pub fn otx(&self) -> Result<Option<OtxView>, CoreError> {
        match &self.inner {
            CobuildWitnessLayout::Otx(otx) => otx_view(otx).map(Some),
            _ => Ok(None),
        }
    }
}

fn verify_message(message: &Cursor) -> Result<(), CoreError> {
    let message = Message::from(message.clone());
    message
        .verify(false)
        .map_err(|_| CoreError::InvalidOtxLayout)
}

fn parse_actions_from_verified_message(message: &Cursor) -> Result<Vec<ActionView>, CoreError> {
    let message = Message::from(message.clone());
    let actions = message.actions().map_err(|_| CoreError::MalformedCobuild)?;
    let action_count = actions.len().map_err(|_| CoreError::MalformedCobuild)?;
    let mut out = Vec::with_capacity(action_count);
    for index in 0..action_count {
        let action = actions
            .get(index)
            .map_err(|_| CoreError::MalformedCobuild)?;
        out.push(action_view(index, action)?);
    }
    Ok(out)
}

fn parse_action_from_verified_message(
    message: &Cursor,
    index: usize,
) -> Result<Option<ActionView>, CoreError> {
    let message = Message::from(message.clone());
    let actions = message.actions().map_err(|_| CoreError::MalformedCobuild)?;
    let action_count = actions.len().map_err(|_| CoreError::MalformedCobuild)?;
    if index >= action_count {
        return Ok(None);
    }
    let action = actions
        .get(index)
        .map_err(|_| CoreError::MalformedCobuild)?;
    action_view(index, action).map(Some)
}

fn action_view(index: usize, action: Action) -> Result<ActionView, CoreError> {
    let script_role = action
        .script_role()
        .map_err(|_| CoreError::MalformedCobuild)?;
    Ok(ActionView {
        index,
        script_info_hash: action
            .script_info_hash()
            .map_err(|_| CoreError::MalformedCobuild)?,
        script_role: ScriptRole::try_from(script_role)?,
        script_hash: action
            .script_hash()
            .map_err(|_| CoreError::MalformedCobuild)?,
        data: action.data().map_err(|_| CoreError::MalformedCobuild)?,
    })
}

fn otx_start_view(start: &OtxStart) -> Result<OtxStartView, CoreError> {
    Ok(OtxStartView {
        start_input_cell: usize_from_u32(
            start
                .start_input_cell()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        start_output_cell: usize_from_u32(
            start
                .start_output_cell()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        start_cell_deps: usize_from_u32(
            start
                .start_cell_deps()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        start_header_deps: usize_from_u32(
            start
                .start_header_deps()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
    })
}

fn otx_view(otx: &Otx) -> Result<OtxView, CoreError> {
    let append_segments_reader = otx
        .append_segments()
        .map_err(|_| CoreError::MalformedCobuild)?;
    let append_segment_count = append_segments_reader
        .len()
        .map_err(|_| CoreError::MalformedCobuild)?;
    let mut append_segments = Vec::with_capacity(append_segment_count);
    for index in 0..append_segment_count {
        append_segments.push(append_segment_data(
            &append_segments_reader
                .get(index)
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?);
    }

    let base_seals_reader = otx.base_seals().map_err(|_| CoreError::MalformedCobuild)?;
    let base_seal_count = base_seals_reader
        .len()
        .map_err(|_| CoreError::MalformedCobuild)?;
    let mut base_seals = Vec::with_capacity(base_seal_count);
    for index in 0..base_seal_count {
        base_seals.push(lock_seal_data(
            &base_seals_reader
                .get(index)
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?);
    }

    let message = otx.message().map_err(|_| CoreError::MalformedCobuild)?;
    Ok(OtxView {
        message: message.cursor,
        append_permissions: otx
            .append_permissions()
            .map_err(|_| CoreError::MalformedCobuild)?,
        base_input_cells: usize_from_u32(
            otx.base_input_cells()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        base_input_masks: MaskView::new(cursor_bytes(
            &otx.base_input_masks()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?),
        base_output_cells: usize_from_u32(
            otx.base_output_cells()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        base_output_masks: MaskView::new(cursor_bytes(
            &otx.base_output_masks()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?),
        base_cell_deps: usize_from_u32(
            otx.base_cell_deps()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        base_cell_dep_masks: MaskView::new(cursor_bytes(
            &otx.base_cell_dep_masks()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?),
        base_header_deps: usize_from_u32(
            otx.base_header_deps()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        base_header_dep_masks: MaskView::new(cursor_bytes(
            &otx.base_header_dep_masks()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?),
        append_segments,
        base_seals,
    })
}

fn append_segment_data(segment: &OtxAppendSegment) -> Result<OtxAppendSegmentView, CoreError> {
    let seals_reader = segment.seals().map_err(|_| CoreError::MalformedCobuild)?;
    let seal_count = seals_reader
        .len()
        .map_err(|_| CoreError::MalformedCobuild)?;
    let mut seals = Vec::with_capacity(seal_count);
    for index in 0..seal_count {
        seals.push(lock_seal_data(
            &seals_reader
                .get(index)
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?);
    }

    Ok(OtxAppendSegmentView {
        segment_flags: segment
            .segment_flags()
            .map_err(|_| CoreError::MalformedCobuild)?,
        input_cells: usize_from_u32(
            segment
                .input_cells()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        output_cells: usize_from_u32(
            segment
                .output_cells()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        cell_deps: usize_from_u32(
            segment
                .cell_deps()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        header_deps: usize_from_u32(
            segment
                .header_deps()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        seals,
    })
}

fn lock_seal_data(seal: &LockSeal) -> Result<LockSealView, CoreError> {
    Ok(LockSealView {
        script_hash: seal
            .script_hash()
            .map_err(|_| CoreError::MalformedCobuild)?,
        seal: seal.seal().map_err(|_| CoreError::MalformedCobuild)?,
    })
}

fn usize_from_u32(value: u32) -> Result<usize, CoreError> {
    usize::try_from(value).map_err(|_| CoreError::InvalidOtxLayout)
}

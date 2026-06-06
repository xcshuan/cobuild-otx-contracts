use alloc::vec::Vec;

use cobuild_types::lazy_reader::{
    core::{Message, Otx, OtxStart, SealPair},
    support::Cursor,
    witness::WitnessLayout as CobuildWitnessLayout,
};

use crate::{
    error::CoreError,
    protocol::ScriptRole,
    reader::{cursor_bytes, cursor_from_slice},
};

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
pub struct SealPairView {
    pub script_hash: [u8; 32],
    pub scope: u8,
    pub seal: Cursor,
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
    pub append_input_cells: usize,
    pub append_output_cells: usize,
    pub append_cell_deps: usize,
    pub append_header_deps: usize,
    pub seals: Vec<SealPairView>,
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
        parse_actions(&self.cursor)
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

impl CobuildWitnessLayoutView {
    pub fn from_slice(data: &[u8]) -> Result<Self, CoreError> {
        let cursor = cursor_from_slice(data);
        let inner =
            CobuildWitnessLayout::try_from(cursor).map_err(|_| CoreError::MalformedCobuild)?;

        inner
            .verify(false)
            .map_err(|_| CoreError::InvalidOtxLayout)?;

        Ok(Self { inner })
    }

    pub fn sighash_all_only_seal(&self) -> Result<Option<Vec<u8>>, CoreError> {
        match &self.inner {
            CobuildWitnessLayout::SighashAllOnly(witness) => {
                let seal = witness.seal().map_err(|_| CoreError::MalformedCobuild)?;
                cursor_bytes(&seal).map(Some)
            }
            _ => Ok(None),
        }
    }

    pub fn sighash_all_message(&self) -> Result<Option<Cursor>, CoreError> {
        match &self.inner {
            CobuildWitnessLayout::SighashAll(witness) => {
                let message = witness.message().map_err(|_| CoreError::MalformedCobuild)?;
                Ok(Some(message.cursor))
            }
            _ => Ok(None),
        }
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

fn parse_actions(message: &Cursor) -> Result<Vec<ActionView>, CoreError> {
    let message = Message::from(message.clone());
    message
        .verify(false)
        .map_err(|_| CoreError::InvalidOtxLayout)?;
    let actions = message.actions().map_err(|_| CoreError::MalformedCobuild)?;
    let action_count = actions.len().map_err(|_| CoreError::MalformedCobuild)?;
    let mut out = Vec::with_capacity(action_count);
    for index in 0..action_count {
        let action = actions
            .get(index)
            .map_err(|_| CoreError::MalformedCobuild)?;
        let script_role = action
            .script_role()
            .map_err(|_| CoreError::MalformedCobuild)?;
        out.push(ActionView {
            index,
            script_info_hash: action
                .script_info_hash()
                .map_err(|_| CoreError::MalformedCobuild)?,
            script_role: ScriptRole::try_from(script_role)?,
            script_hash: action
                .script_hash()
                .map_err(|_| CoreError::MalformedCobuild)?,
            data: action.data().map_err(|_| CoreError::MalformedCobuild)?,
        });
    }
    Ok(out)
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
    let seals_reader = otx.seals().map_err(|_| CoreError::MalformedCobuild)?;
    let seal_count = seals_reader
        .len()
        .map_err(|_| CoreError::MalformedCobuild)?;
    let mut seals = Vec::with_capacity(seal_count);
    for index in 0..seal_count {
        seals.push(seal_pair_data(
            &seals_reader
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
        append_input_cells: usize_from_u32(
            otx.append_input_cells()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        append_output_cells: usize_from_u32(
            otx.append_output_cells()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        append_cell_deps: usize_from_u32(
            otx.append_cell_deps()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        append_header_deps: usize_from_u32(
            otx.append_header_deps()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        seals,
    })
}

fn seal_pair_data(pair: &SealPair) -> Result<SealPairView, CoreError> {
    Ok(SealPairView {
        script_hash: pair
            .script_hash()
            .map_err(|_| CoreError::MalformedCobuild)?,
        scope: pair.scope().map_err(|_| CoreError::MalformedCobuild)?,
        seal: pair.seal().map_err(|_| CoreError::MalformedCobuild)?,
    })
}

fn usize_from_u32(value: u32) -> Result<usize, CoreError> {
    usize::try_from(value).map_err(|_| CoreError::InvalidOtxLayout)
}

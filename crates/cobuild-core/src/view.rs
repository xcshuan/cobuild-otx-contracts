use alloc::vec::Vec;

use cobuild_types::lazy_reader::{
    core::{Message, Otx, OtxStart, SealPair},
    support::Cursor,
    witness::WitnessLayout,
};

use crate::{
    error::CoreError,
    reader::{cursor_bytes, cursor_from_slice},
};

pub struct WitnessLayoutView {
    #[allow(dead_code)]
    pub(crate) inner: WitnessLayout,
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
    cursor: Cursor,
}

#[derive(Clone)]
pub struct SealPairView {
    pub script_hash: [u8; 32],
    pub scope: u8,
    pub seal: Cursor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MessageActionView {
    pub script_role: u8,
    pub script_hash: [u8; 32],
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
}

impl From<Cursor> for MessageView {
    fn from(cursor: Cursor) -> Self {
        Self::new(cursor)
    }
}

impl MaskView {
    pub fn new(cursor: Cursor) -> Self {
        Self { cursor }
    }

    pub fn bit(&self, index: usize) -> Result<bool, CoreError> {
        let byte = self.byte(index / 8)?;
        Ok(byte & (1 << (index % 8)) != 0)
    }

    pub fn len(&self) -> usize {
        self.cursor.size
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    fn byte(&self, index: usize) -> Result<u8, CoreError> {
        if index >= self.cursor.size {
            return Err(CoreError::InvalidOtxLayout);
        }
        let mut byte_cursor = self.cursor.clone();
        byte_cursor
            .add_offset(index)
            .map_err(|_| CoreError::InvalidOtxLayout)?;
        byte_cursor.size = 1;

        let mut out = [0u8; 1];
        let read = byte_cursor
            .read_at(&mut out)
            .map_err(|_| CoreError::InvalidOtxLayout)?;
        if read != out.len() {
            return Err(CoreError::InvalidOtxLayout);
        }
        Ok(out[0])
    }
}

impl WitnessLayoutView {
    pub fn from_slice(data: &[u8]) -> Result<Self, CoreError> {
        let cursor = cursor_from_slice(data);
        let inner = WitnessLayout::try_from(cursor).map_err(|_| CoreError::MalformedCobuild)?;

        inner
            .verify(false)
            .map_err(|_| CoreError::InvalidOtxLayout)?;

        Ok(Self { inner })
    }

    pub fn sighash_all_only_seal(&self) -> Result<Option<Vec<u8>>, CoreError> {
        match &self.inner {
            WitnessLayout::SighashAllOnly(witness) => {
                let seal = witness.seal().map_err(|_| CoreError::MalformedCobuild)?;
                cursor_bytes(&seal).map(Some)
            }
            _ => Ok(None),
        }
    }

    pub fn sighash_all_message(&self) -> Result<Option<Cursor>, CoreError> {
        match &self.inner {
            WitnessLayout::SighashAll(witness) => {
                let message = witness.message().map_err(|_| CoreError::MalformedCobuild)?;
                Ok(Some(message.cursor))
            }
            _ => Ok(None),
        }
    }

    pub fn sighash_all_witness_layout(&self) -> Result<Option<SighashAllWitnessView>, CoreError> {
        match &self.inner {
            WitnessLayout::SighashAll(witness) => {
                let seal = witness.seal().map_err(|_| CoreError::MalformedCobuild)?;
                let message = witness.message().map_err(|_| CoreError::MalformedCobuild)?;
                Ok(Some(SighashAllWitnessView::WithMessage {
                    seal,
                    message: message.cursor,
                }))
            }
            WitnessLayout::SighashAllOnly(witness) => witness
                .seal()
                .map(|seal| Some(SighashAllWitnessView::SealOnly { seal }))
                .map_err(|_| CoreError::MalformedCobuild),
            _ => Ok(None),
        }
    }

    pub fn otx_start(&self) -> Result<Option<OtxStartView>, CoreError> {
        match &self.inner {
            WitnessLayout::OtxStart(start) => otx_start_view(start).map(Some),
            _ => Ok(None),
        }
    }

    pub fn otx(&self) -> Result<Option<OtxView>, CoreError> {
        match &self.inner {
            WitnessLayout::Otx(otx) => otx_view(otx).map(Some),
            _ => Ok(None),
        }
    }
}

pub(crate) fn message_actions(message: &Cursor) -> Result<Vec<MessageActionView>, CoreError> {
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
        out.push(MessageActionView {
            script_role: action
                .script_role()
                .map_err(|_| CoreError::MalformedCobuild)?,
            script_hash: action
                .script_hash()
                .map_err(|_| CoreError::MalformedCobuild)?,
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
        base_input_masks: MaskView::new(
            otx.base_input_masks()
                .map_err(|_| CoreError::MalformedCobuild)?,
        ),
        base_output_cells: usize_from_u32(
            otx.base_output_cells()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        base_output_masks: MaskView::new(
            otx.base_output_masks()
                .map_err(|_| CoreError::MalformedCobuild)?,
        ),
        base_cell_deps: usize_from_u32(
            otx.base_cell_deps()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        base_cell_dep_masks: MaskView::new(
            otx.base_cell_dep_masks()
                .map_err(|_| CoreError::MalformedCobuild)?,
        ),
        base_header_deps: usize_from_u32(
            otx.base_header_deps()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        base_header_dep_masks: MaskView::new(
            otx.base_header_dep_masks()
                .map_err(|_| CoreError::MalformedCobuild)?,
        ),
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

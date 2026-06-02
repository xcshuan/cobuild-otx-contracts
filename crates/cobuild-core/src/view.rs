use alloc::{boxed::Box, vec, vec::Vec};
use core::{cmp::min, convert::TryInto};

use cobuild_types::lazy_reader::{
    core::{Message, Otx, OtxStart, SealPair},
    support::{Cursor, Error as MoleculeError, Read},
    witness::WitnessLayout,
};

use crate::error::CoreError;

pub struct OwnedReader {
    data: Vec<u8>,
}

impl OwnedReader {
    pub fn new(data: &[u8]) -> Self {
        Self {
            data: data.to_vec(),
        }
    }
}

impl Read for OwnedReader {
    fn read(&self, buf: &mut [u8], offset: usize) -> Result<usize, MoleculeError> {
        if offset >= self.data.len() {
            return Err(MoleculeError::OutOfBound(offset, self.data.len()));
        }

        let read_len = min(buf.len(), self.data.len() - offset);
        buf[..read_len].copy_from_slice(&self.data[offset..offset + read_len]);
        Ok(read_len)
    }
}

pub struct WitnessLayoutView {
    #[allow(dead_code)]
    pub(crate) inner: WitnessLayout,
}

pub enum SighashAllWitnessLayout {
    WithMessage { seal: Vec<u8>, message: Vec<u8> },
    SealOnly { seal: Vec<u8> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OtxStartData {
    pub start_input_cell: usize,
    pub start_output_cell: usize,
    pub start_cell_deps: usize,
    pub start_header_deps: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealPairData {
    pub script_hash: [u8; 32],
    pub scope: u8,
    pub seal: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionData {
    pub script_role: u8,
    pub script_hash: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OtxData {
    pub message: Vec<u8>,
    pub append_permissions: u8,
    pub base_input_cells: usize,
    pub base_input_masks: Vec<u8>,
    pub base_output_cells: usize,
    pub base_output_masks: Vec<u8>,
    pub base_cell_deps: usize,
    pub base_cell_dep_masks: Vec<u8>,
    pub base_header_deps: usize,
    pub base_header_dep_masks: Vec<u8>,
    pub append_input_cells: usize,
    pub append_output_cells: usize,
    pub append_cell_deps: usize,
    pub append_header_deps: usize,
    pub seals: Vec<SealPairData>,
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
            WitnessLayout::SighashAllOnly(witness) => witness
                .seal()
                .and_then(TryInto::try_into)
                .map(Some)
                .map_err(|_| CoreError::MalformedCobuild),
            _ => Ok(None),
        }
    }

    pub fn sighash_all_message(&self) -> Result<Option<Vec<u8>>, CoreError> {
        match &self.inner {
            WitnessLayout::SighashAll(witness) => {
                let message = witness.message().map_err(|_| CoreError::MalformedCobuild)?;
                cursor_bytes(&message.cursor).map(Some)
            }
            _ => Ok(None),
        }
    }

    pub fn sighash_all_witness_layout(&self) -> Result<Option<SighashAllWitnessLayout>, CoreError> {
        match &self.inner {
            WitnessLayout::SighashAll(witness) => {
                let seal = witness
                    .seal()
                    .and_then(|cursor| cursor.try_into())
                    .map_err(|_| CoreError::MalformedCobuild)?;
                let message = witness.message().map_err(|_| CoreError::MalformedCobuild)?;
                Ok(Some(SighashAllWitnessLayout::WithMessage {
                    seal,
                    message: cursor_bytes(&message.cursor)?,
                }))
            }
            WitnessLayout::SighashAllOnly(witness) => witness
                .seal()
                .and_then(TryInto::try_into)
                .map(|seal| Some(SighashAllWitnessLayout::SealOnly { seal }))
                .map_err(|_| CoreError::MalformedCobuild),
            _ => Ok(None),
        }
    }

    pub fn otx_start(&self) -> Result<Option<OtxStartData>, CoreError> {
        match &self.inner {
            WitnessLayout::OtxStart(start) => otx_start_data(start).map(Some),
            _ => Ok(None),
        }
    }

    pub fn otx(&self) -> Result<Option<OtxData>, CoreError> {
        match &self.inner {
            WitnessLayout::Otx(otx) => otx_data(otx).map(Some),
            _ => Ok(None),
        }
    }
}

pub(crate) fn cursor_bytes(cursor: &Cursor) -> Result<Vec<u8>, CoreError> {
    let mut bytes = vec![0; cursor.size];
    let read = cursor
        .read_at(&mut bytes)
        .map_err(|_| CoreError::MalformedCobuild)?;
    if read != bytes.len() {
        return Err(CoreError::MalformedCobuild);
    }
    Ok(bytes)
}

pub(crate) fn update_cursor(
    hasher: &mut blake2b_ref::Blake2b,
    cursor: &Cursor,
) -> Result<(), CoreError> {
    let mut offset = 0usize;
    let mut buf = [0u8; 256];

    while offset < cursor.size {
        let read_len = min(buf.len(), cursor.size - offset);
        let mut chunk = cursor.clone();
        chunk
            .add_offset(offset)
            .map_err(|_| CoreError::MalformedCobuild)?;
        chunk.size = read_len;

        let read = chunk
            .read_at(&mut buf[..read_len])
            .map_err(|_| CoreError::MalformedCobuild)?;
        if read != read_len {
            return Err(CoreError::MalformedCobuild);
        }

        hasher.update(&buf[..read_len]);
        offset = offset
            .checked_add(read_len)
            .ok_or(CoreError::MalformedCobuild)?;
    }

    Ok(())
}

pub(crate) fn message_actions(message_bytes: &[u8]) -> Result<Vec<ActionData>, CoreError> {
    let message = Message::from(cursor_from_slice(message_bytes));
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
        out.push(ActionData {
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

fn otx_start_data(start: &OtxStart) -> Result<OtxStartData, CoreError> {
    Ok(OtxStartData {
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

fn otx_data(otx: &Otx) -> Result<OtxData, CoreError> {
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
    Ok(OtxData {
        message: cursor_bytes(&message.cursor)?,
        append_permissions: otx
            .append_permissions()
            .map_err(|_| CoreError::MalformedCobuild)?,
        base_input_cells: usize_from_u32(
            otx.base_input_cells()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        base_input_masks: cursor_bytes(
            &otx.base_input_masks()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        base_output_cells: usize_from_u32(
            otx.base_output_cells()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        base_output_masks: cursor_bytes(
            &otx.base_output_masks()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        base_cell_deps: usize_from_u32(
            otx.base_cell_deps()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        base_cell_dep_masks: cursor_bytes(
            &otx.base_cell_dep_masks()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        base_header_deps: usize_from_u32(
            otx.base_header_deps()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
        base_header_dep_masks: cursor_bytes(
            &otx.base_header_dep_masks()
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?,
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

fn seal_pair_data(pair: &SealPair) -> Result<SealPairData, CoreError> {
    Ok(SealPairData {
        script_hash: pair
            .script_hash()
            .map_err(|_| CoreError::MalformedCobuild)?,
        scope: pair.scope().map_err(|_| CoreError::MalformedCobuild)?,
        seal: cursor_bytes(&pair.seal().map_err(|_| CoreError::MalformedCobuild)?)?,
    })
}

fn usize_from_u32(value: u32) -> Result<usize, CoreError> {
    usize::try_from(value).map_err(|_| CoreError::InvalidOtxLayout)
}

pub(crate) fn cursor_from_slice(data: &[u8]) -> Cursor {
    let reader: Box<dyn Read> = Box::new(OwnedReader::new(data));
    Cursor::new(data.len(), reader)
}

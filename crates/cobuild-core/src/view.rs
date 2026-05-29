use alloc::{boxed::Box, vec, vec::Vec};
use core::{cmp::min, convert::TryInto, marker::PhantomData};

use cobuild_types::lazy_reader::{
    support::{Cursor, Error as MoleculeError, Read},
    witness::WitnessLayout,
};

use crate::error::CoreError;

pub struct SliceReader<'a> {
    data: &'a [u8],
}

impl<'a> SliceReader<'a> {
    pub const fn new(data: &'a [u8]) -> Self {
        Self { data }
    }
}

impl Read for SliceReader<'_> {
    fn read(&self, buf: &mut [u8], offset: usize) -> Result<usize, MoleculeError> {
        if offset >= self.data.len() {
            return Err(MoleculeError::OutOfBound(offset, self.data.len()));
        }

        let read_len = min(buf.len(), self.data.len() - offset);
        buf[..read_len].copy_from_slice(&self.data[offset..offset + read_len]);
        Ok(read_len)
    }
}

pub struct WitnessLayoutView<'a> {
    #[allow(dead_code)]
    pub(crate) inner: WitnessLayout,
    _data: PhantomData<&'a [u8]>,
}

pub enum TxLevelWitness {
    SighashAll { seal: Vec<u8>, message: Vec<u8> },
    SighashAllOnly { seal: Vec<u8> },
}

impl<'a> WitnessLayoutView<'a> {
    pub fn from_slice(data: &'a [u8]) -> Result<Self, CoreError> {
        let cursor = cursor_from_slice(data);
        let inner = WitnessLayout::try_from(cursor).map_err(|_| CoreError::MalformedCobuild)?;

        inner.verify(false).map_err(|_| CoreError::InvalidLayout)?;

        Ok(Self {
            inner,
            _data: PhantomData,
        })
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

    pub fn tx_level_witness(&self) -> Result<Option<TxLevelWitness>, CoreError> {
        match &self.inner {
            WitnessLayout::SighashAll(witness) => {
                let seal = witness
                    .seal()
                    .and_then(|cursor| cursor.try_into())
                    .map_err(|_| CoreError::MalformedCobuild)?;
                let message = witness.message().map_err(|_| CoreError::MalformedCobuild)?;
                Ok(Some(TxLevelWitness::SighashAll {
                    seal,
                    message: cursor_bytes(&message.cursor)?,
                }))
            }
            WitnessLayout::SighashAllOnly(witness) => witness
                .seal()
                .and_then(TryInto::try_into)
                .map(|seal| Some(TxLevelWitness::SighashAllOnly { seal }))
                .map_err(|_| CoreError::MalformedCobuild),
            _ => Ok(None),
        }
    }
}

fn cursor_bytes(cursor: &Cursor) -> Result<Vec<u8>, CoreError> {
    let mut bytes = vec![0; cursor.size];
    let read = cursor
        .read_at(&mut bytes)
        .map_err(|_| CoreError::MalformedCobuild)?;
    if read != bytes.len() {
        return Err(CoreError::MalformedCobuild);
    }
    Ok(bytes)
}

pub(crate) fn cursor_from_slice<'a>(data: &'a [u8]) -> Cursor {
    let reader: Box<dyn Read + 'a> = Box::new(SliceReader::new(data));
    Cursor::new(data.len(), erase_reader_lifetime(reader))
}

fn erase_reader_lifetime<'a>(reader: Box<dyn Read + 'a>) -> Box<dyn Read> {
    // `WitnessLayoutView` carries the input lifetime, but molecule 0.9.2's
    // cursor stores `Box<dyn Read>` without a lifetime parameter. Keep
    // generated readers crate-private so cloneable cursors cannot escape the
    // view lifetime through the public API.
    unsafe { core::mem::transmute::<Box<dyn Read + 'a>, Box<dyn Read>>(reader) }
}

use alloc::boxed::Box;
use core::{cmp::min, marker::PhantomData};

use cobuild_types::lazy_reader::witness::WitnessLayout;
use molecule::lazy_reader::{Cursor, Error as MoleculeError, Read};

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

impl<'a> WitnessLayoutView<'a> {
    pub fn from_slice(data: &'a [u8]) -> Result<Self, CoreError> {
        let reader: Box<dyn Read + 'a> = Box::new(SliceReader::new(data));
        let cursor = Cursor::new(data.len(), erase_reader_lifetime(reader));
        let inner = WitnessLayout::try_from(cursor).map_err(|_| CoreError::MalformedCobuild)?;

        inner.verify(false).map_err(|_| CoreError::InvalidLayout)?;

        Ok(Self {
            inner,
            _data: PhantomData,
        })
    }

}

fn erase_reader_lifetime<'a>(reader: Box<dyn Read + 'a>) -> Box<dyn Read> {
    // `WitnessLayoutView` carries the input lifetime, but molecule 0.9.2's
    // cursor stores `Box<dyn Read>` without a lifetime parameter. Keep
    // generated readers crate-private so cloneable cursors cannot escape the
    // view lifetime through the public API.
    unsafe { core::mem::transmute::<Box<dyn Read + 'a>, Box<dyn Read>>(reader) }
}

extern crate alloc;
use super::blockchain::*;
use super::support::{Cursor, Error, NUMBER_SIZE};
use core::convert::TryInto;
#[derive(Clone)]
pub struct Action {
    pub cursor: Cursor,
}
impl From<Cursor> for Action {
    fn from(cursor: Cursor) -> Self {
        Action { cursor }
    }
}
impl Action {
    pub fn script_info_hash(&self) -> Result<[u8; 32usize], Error> {
        let cur = self.cursor.table_slice_by_index(0usize)?;
        cur.try_into()
    }
}
impl Action {
    pub fn script_role(&self) -> Result<u8, Error> {
        let cur = self.cursor.table_slice_by_index(1usize)?;
        cur.try_into()
    }
}
impl Action {
    pub fn script_hash(&self) -> Result<[u8; 32usize], Error> {
        let cur = self.cursor.table_slice_by_index(2usize)?;
        cur.try_into()
    }
}
impl Action {
    pub fn data(&self) -> Result<Cursor, Error> {
        let cur = self.cursor.table_slice_by_index(3usize)?;
        cur.convert_to_rawbytes()
    }
}
impl Action {
    pub fn verify(&self, compatible: bool) -> Result<(), Error> {
        self.cursor.verify_table(4usize, compatible)?;
        Byte32::from(Cursor::try_from(self.script_info_hash()?)?).verify(compatible)?;
        Byte32::from(Cursor::try_from(self.script_hash()?)?).verify(compatible)?;
        Ok(())
    }
}
#[derive(Clone)]
pub struct ActionVec {
    pub cursor: Cursor,
}
impl From<Cursor> for ActionVec {
    fn from(cursor: Cursor) -> Self {
        Self { cursor }
    }
}
impl ActionVec {
    pub fn len(&self) -> Result<usize, Error> {
        self.cursor.dynvec_length()
    }
}
impl ActionVec {
    pub fn get(&self, index: usize) -> Result<Action, Error> {
        let cur = self.cursor.dynvec_slice_by_index(index)?;
        Ok(cur.into())
    }
}
pub struct ActionVecIterator {
    cur: ActionVec,
    index: usize,
    len: usize,
}
impl core::iter::Iterator for ActionVecIterator {
    type Item = Action;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            None
        } else {
            let res = self.cur.get(self.index).unwrap();
            self.index += 1;
            Some(res)
        }
    }
}
impl core::iter::IntoIterator for ActionVec {
    type Item = Action;
    type IntoIter = ActionVecIterator;
    fn into_iter(self) -> Self::IntoIter {
        let len = self.len().unwrap();
        Self::IntoIter {
            cur: self,
            index: 0,
            len,
        }
    }
}
pub struct ActionVecIteratorRef<'a> {
    cur: &'a ActionVec,
    index: usize,
    len: usize,
}
impl<'a> core::iter::Iterator for ActionVecIteratorRef<'a> {
    type Item = Action;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            None
        } else {
            let res = self.cur.get(self.index).unwrap();
            self.index += 1;
            Some(res)
        }
    }
}
impl ActionVec {
    pub fn iter(&self) -> ActionVecIteratorRef {
        let len = self.len().unwrap();
        ActionVecIteratorRef {
            cur: &self,
            index: 0,
            len,
        }
    }
}
impl ActionVec {
    pub fn verify(&self, compatible: bool) -> Result<(), Error> {
        self.cursor.verify_dynvec()?;
        for i in 0..self.len()? {
            self.get(i)?.verify(compatible)?;
        }
        Ok(())
    }
}
#[derive(Clone)]
pub struct Message {
    pub cursor: Cursor,
}
impl From<Cursor> for Message {
    fn from(cursor: Cursor) -> Self {
        Message { cursor }
    }
}
impl Message {
    pub fn actions(&self) -> Result<ActionVec, Error> {
        let cur = self.cursor.table_slice_by_index(0usize)?;
        Ok(cur.into())
    }
}
impl Message {
    pub fn verify(&self, compatible: bool) -> Result<(), Error> {
        self.cursor.verify_table(1usize, compatible)?;
        self.actions()?.verify(compatible)?;
        Ok(())
    }
}
#[derive(Clone)]
pub struct SighashAll {
    pub cursor: Cursor,
}
impl From<Cursor> for SighashAll {
    fn from(cursor: Cursor) -> Self {
        SighashAll { cursor }
    }
}
impl SighashAll {
    pub fn seal(&self) -> Result<Cursor, Error> {
        let cur = self.cursor.table_slice_by_index(0usize)?;
        cur.convert_to_rawbytes()
    }
}
impl SighashAll {
    pub fn message(&self) -> Result<Message, Error> {
        let cur = self.cursor.table_slice_by_index(1usize)?;
        Ok(cur.into())
    }
}
impl SighashAll {
    pub fn verify(&self, compatible: bool) -> Result<(), Error> {
        self.cursor.verify_table(2usize, compatible)?;
        self.message()?.verify(compatible)?;
        Ok(())
    }
}
#[derive(Clone)]
pub struct SighashAllOnly {
    pub cursor: Cursor,
}
impl From<Cursor> for SighashAllOnly {
    fn from(cursor: Cursor) -> Self {
        SighashAllOnly { cursor }
    }
}
impl SighashAllOnly {
    pub fn seal(&self) -> Result<Cursor, Error> {
        let cur = self.cursor.table_slice_by_index(0usize)?;
        cur.convert_to_rawbytes()
    }
}
impl SighashAllOnly {
    pub fn verify(&self, compatible: bool) -> Result<(), Error> {
        self.cursor.verify_table(1usize, compatible)?;
        Ok(())
    }
}
#[derive(Clone)]
pub struct SealPair {
    pub cursor: Cursor,
}
impl From<Cursor> for SealPair {
    fn from(cursor: Cursor) -> Self {
        SealPair { cursor }
    }
}
impl SealPair {
    pub fn script_hash(&self) -> Result<[u8; 32usize], Error> {
        let cur = self.cursor.table_slice_by_index(0usize)?;
        cur.try_into()
    }
}
impl SealPair {
    pub fn scope(&self) -> Result<u8, Error> {
        let cur = self.cursor.table_slice_by_index(1usize)?;
        cur.try_into()
    }
}
impl SealPair {
    pub fn seal(&self) -> Result<Cursor, Error> {
        let cur = self.cursor.table_slice_by_index(2usize)?;
        cur.convert_to_rawbytes()
    }
}
impl SealPair {
    pub fn verify(&self, compatible: bool) -> Result<(), Error> {
        self.cursor.verify_table(3usize, compatible)?;
        Byte32::from(Cursor::try_from(self.script_hash()?)?).verify(compatible)?;
        Ok(())
    }
}
#[derive(Clone)]
pub struct SealPairVec {
    pub cursor: Cursor,
}
impl From<Cursor> for SealPairVec {
    fn from(cursor: Cursor) -> Self {
        Self { cursor }
    }
}
impl SealPairVec {
    pub fn len(&self) -> Result<usize, Error> {
        self.cursor.dynvec_length()
    }
}
impl SealPairVec {
    pub fn get(&self, index: usize) -> Result<SealPair, Error> {
        let cur = self.cursor.dynvec_slice_by_index(index)?;
        Ok(cur.into())
    }
}
pub struct SealPairVecIterator {
    cur: SealPairVec,
    index: usize,
    len: usize,
}
impl core::iter::Iterator for SealPairVecIterator {
    type Item = SealPair;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            None
        } else {
            let res = self.cur.get(self.index).unwrap();
            self.index += 1;
            Some(res)
        }
    }
}
impl core::iter::IntoIterator for SealPairVec {
    type Item = SealPair;
    type IntoIter = SealPairVecIterator;
    fn into_iter(self) -> Self::IntoIter {
        let len = self.len().unwrap();
        Self::IntoIter {
            cur: self,
            index: 0,
            len,
        }
    }
}
pub struct SealPairVecIteratorRef<'a> {
    cur: &'a SealPairVec,
    index: usize,
    len: usize,
}
impl<'a> core::iter::Iterator for SealPairVecIteratorRef<'a> {
    type Item = SealPair;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            None
        } else {
            let res = self.cur.get(self.index).unwrap();
            self.index += 1;
            Some(res)
        }
    }
}
impl SealPairVec {
    pub fn iter(&self) -> SealPairVecIteratorRef {
        let len = self.len().unwrap();
        SealPairVecIteratorRef {
            cur: &self,
            index: 0,
            len,
        }
    }
}
impl SealPairVec {
    pub fn verify(&self, compatible: bool) -> Result<(), Error> {
        self.cursor.verify_dynvec()?;
        for i in 0..self.len()? {
            self.get(i)?.verify(compatible)?;
        }
        Ok(())
    }
}
#[derive(Clone)]
pub struct OtxStart {
    pub cursor: Cursor,
}
impl From<Cursor> for OtxStart {
    fn from(cursor: Cursor) -> Self {
        OtxStart { cursor }
    }
}
impl OtxStart {
    pub fn start_input_cell(&self) -> Result<u32, Error> {
        let cur = self.cursor.table_slice_by_index(0usize)?;
        cur.try_into()
    }
}
impl OtxStart {
    pub fn start_output_cell(&self) -> Result<u32, Error> {
        let cur = self.cursor.table_slice_by_index(1usize)?;
        cur.try_into()
    }
}
impl OtxStart {
    pub fn start_cell_deps(&self) -> Result<u32, Error> {
        let cur = self.cursor.table_slice_by_index(2usize)?;
        cur.try_into()
    }
}
impl OtxStart {
    pub fn start_header_deps(&self) -> Result<u32, Error> {
        let cur = self.cursor.table_slice_by_index(3usize)?;
        cur.try_into()
    }
}
impl OtxStart {
    pub fn verify(&self, compatible: bool) -> Result<(), Error> {
        self.cursor.verify_table(4usize, compatible)?;
        Ok(())
    }
}
#[derive(Clone)]
pub struct Otx {
    pub cursor: Cursor,
}
impl From<Cursor> for Otx {
    fn from(cursor: Cursor) -> Self {
        Otx { cursor }
    }
}
impl Otx {
    pub fn message(&self) -> Result<Message, Error> {
        let cur = self.cursor.table_slice_by_index(0usize)?;
        Ok(cur.into())
    }
}
impl Otx {
    pub fn append_permissions(&self) -> Result<u8, Error> {
        let cur = self.cursor.table_slice_by_index(1usize)?;
        cur.try_into()
    }
}
impl Otx {
    pub fn base_input_cells(&self) -> Result<u32, Error> {
        let cur = self.cursor.table_slice_by_index(2usize)?;
        cur.try_into()
    }
}
impl Otx {
    pub fn base_input_masks(&self) -> Result<Cursor, Error> {
        let cur = self.cursor.table_slice_by_index(3usize)?;
        cur.convert_to_rawbytes()
    }
}
impl Otx {
    pub fn base_output_cells(&self) -> Result<u32, Error> {
        let cur = self.cursor.table_slice_by_index(4usize)?;
        cur.try_into()
    }
}
impl Otx {
    pub fn base_output_masks(&self) -> Result<Cursor, Error> {
        let cur = self.cursor.table_slice_by_index(5usize)?;
        cur.convert_to_rawbytes()
    }
}
impl Otx {
    pub fn base_cell_deps(&self) -> Result<u32, Error> {
        let cur = self.cursor.table_slice_by_index(6usize)?;
        cur.try_into()
    }
}
impl Otx {
    pub fn base_cell_dep_masks(&self) -> Result<Cursor, Error> {
        let cur = self.cursor.table_slice_by_index(7usize)?;
        cur.convert_to_rawbytes()
    }
}
impl Otx {
    pub fn base_header_deps(&self) -> Result<u32, Error> {
        let cur = self.cursor.table_slice_by_index(8usize)?;
        cur.try_into()
    }
}
impl Otx {
    pub fn base_header_dep_masks(&self) -> Result<Cursor, Error> {
        let cur = self.cursor.table_slice_by_index(9usize)?;
        cur.convert_to_rawbytes()
    }
}
impl Otx {
    pub fn append_input_cells(&self) -> Result<u32, Error> {
        let cur = self.cursor.table_slice_by_index(10usize)?;
        cur.try_into()
    }
}
impl Otx {
    pub fn append_output_cells(&self) -> Result<u32, Error> {
        let cur = self.cursor.table_slice_by_index(11usize)?;
        cur.try_into()
    }
}
impl Otx {
    pub fn append_cell_deps(&self) -> Result<u32, Error> {
        let cur = self.cursor.table_slice_by_index(12usize)?;
        cur.try_into()
    }
}
impl Otx {
    pub fn append_header_deps(&self) -> Result<u32, Error> {
        let cur = self.cursor.table_slice_by_index(13usize)?;
        cur.try_into()
    }
}
impl Otx {
    pub fn seals(&self) -> Result<SealPairVec, Error> {
        let cur = self.cursor.table_slice_by_index(14usize)?;
        Ok(cur.into())
    }
}
impl Otx {
    pub fn verify(&self, compatible: bool) -> Result<(), Error> {
        self.cursor.verify_table(15usize, compatible)?;
        self.message()?.verify(compatible)?;
        self.seals()?.verify(compatible)?;
        Ok(())
    }
}

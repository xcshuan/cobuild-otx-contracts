extern crate alloc;
use super::blockchain::*;
use super::support::{Cursor, Error};
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
    pub fn iter(&self) -> ActionVecIteratorRef<'_> {
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
pub struct LockSeal {
    pub cursor: Cursor,
}
impl From<Cursor> for LockSeal {
    fn from(cursor: Cursor) -> Self {
        LockSeal { cursor }
    }
}
impl LockSeal {
    pub fn script_hash(&self) -> Result<[u8; 32usize], Error> {
        let cur = self.cursor.table_slice_by_index(0usize)?;
        cur.try_into()
    }
}
impl LockSeal {
    pub fn seal(&self) -> Result<Cursor, Error> {
        let cur = self.cursor.table_slice_by_index(1usize)?;
        cur.convert_to_rawbytes()
    }
}
impl LockSeal {
    pub fn verify(&self, compatible: bool) -> Result<(), Error> {
        self.cursor.verify_table(2usize, compatible)?;
        Byte32::from(Cursor::try_from(self.script_hash()?)?).verify(compatible)?;
        Ok(())
    }
}
#[derive(Clone)]
pub struct LockSealVec {
    pub cursor: Cursor,
}
impl From<Cursor> for LockSealVec {
    fn from(cursor: Cursor) -> Self {
        Self { cursor }
    }
}
impl LockSealVec {
    pub fn len(&self) -> Result<usize, Error> {
        self.cursor.dynvec_length()
    }
}
impl LockSealVec {
    pub fn get(&self, index: usize) -> Result<LockSeal, Error> {
        let cur = self.cursor.dynvec_slice_by_index(index)?;
        Ok(cur.into())
    }
}
pub struct LockSealVecIterator {
    cur: LockSealVec,
    index: usize,
    len: usize,
}
impl core::iter::Iterator for LockSealVecIterator {
    type Item = LockSeal;
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
impl core::iter::IntoIterator for LockSealVec {
    type Item = LockSeal;
    type IntoIter = LockSealVecIterator;
    fn into_iter(self) -> Self::IntoIter {
        let len = self.len().unwrap();
        Self::IntoIter {
            cur: self,
            index: 0,
            len,
        }
    }
}
pub struct LockSealVecIteratorRef<'a> {
    cur: &'a LockSealVec,
    index: usize,
    len: usize,
}
impl<'a> core::iter::Iterator for LockSealVecIteratorRef<'a> {
    type Item = LockSeal;
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
impl LockSealVec {
    pub fn iter(&self) -> LockSealVecIteratorRef<'_> {
        let len = self.len().unwrap();
        LockSealVecIteratorRef {
            cur: &self,
            index: 0,
            len,
        }
    }
}
impl LockSealVec {
    pub fn verify(&self, compatible: bool) -> Result<(), Error> {
        self.cursor.verify_dynvec()?;
        for i in 0..self.len()? {
            self.get(i)?.verify(compatible)?;
        }
        Ok(())
    }
}
#[derive(Clone)]
pub struct OtxAppendSegment {
    pub cursor: Cursor,
}
impl From<Cursor> for OtxAppendSegment {
    fn from(cursor: Cursor) -> Self {
        OtxAppendSegment { cursor }
    }
}
impl OtxAppendSegment {
    pub fn segment_flags(&self) -> Result<u8, Error> {
        let cur = self.cursor.table_slice_by_index(0usize)?;
        cur.try_into()
    }
}
impl OtxAppendSegment {
    pub fn input_cells(&self) -> Result<u32, Error> {
        let cur = self.cursor.table_slice_by_index(1usize)?;
        cur.try_into()
    }
}
impl OtxAppendSegment {
    pub fn output_cells(&self) -> Result<u32, Error> {
        let cur = self.cursor.table_slice_by_index(2usize)?;
        cur.try_into()
    }
}
impl OtxAppendSegment {
    pub fn cell_deps(&self) -> Result<u32, Error> {
        let cur = self.cursor.table_slice_by_index(3usize)?;
        cur.try_into()
    }
}
impl OtxAppendSegment {
    pub fn header_deps(&self) -> Result<u32, Error> {
        let cur = self.cursor.table_slice_by_index(4usize)?;
        cur.try_into()
    }
}
impl OtxAppendSegment {
    pub fn seals(&self) -> Result<LockSealVec, Error> {
        let cur = self.cursor.table_slice_by_index(5usize)?;
        Ok(cur.into())
    }
}
impl OtxAppendSegment {
    pub fn verify(&self, compatible: bool) -> Result<(), Error> {
        self.cursor.verify_table(6usize, compatible)?;
        self.seals()?.verify(compatible)?;
        Ok(())
    }
}
#[derive(Clone)]
pub struct OtxAppendSegmentVec {
    pub cursor: Cursor,
}
impl From<Cursor> for OtxAppendSegmentVec {
    fn from(cursor: Cursor) -> Self {
        Self { cursor }
    }
}
impl OtxAppendSegmentVec {
    pub fn len(&self) -> Result<usize, Error> {
        self.cursor.dynvec_length()
    }
}
impl OtxAppendSegmentVec {
    pub fn get(&self, index: usize) -> Result<OtxAppendSegment, Error> {
        let cur = self.cursor.dynvec_slice_by_index(index)?;
        Ok(cur.into())
    }
}
pub struct OtxAppendSegmentVecIterator {
    cur: OtxAppendSegmentVec,
    index: usize,
    len: usize,
}
impl core::iter::Iterator for OtxAppendSegmentVecIterator {
    type Item = OtxAppendSegment;
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
impl core::iter::IntoIterator for OtxAppendSegmentVec {
    type Item = OtxAppendSegment;
    type IntoIter = OtxAppendSegmentVecIterator;
    fn into_iter(self) -> Self::IntoIter {
        let len = self.len().unwrap();
        Self::IntoIter {
            cur: self,
            index: 0,
            len,
        }
    }
}
pub struct OtxAppendSegmentVecIteratorRef<'a> {
    cur: &'a OtxAppendSegmentVec,
    index: usize,
    len: usize,
}
impl<'a> core::iter::Iterator for OtxAppendSegmentVecIteratorRef<'a> {
    type Item = OtxAppendSegment;
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
impl OtxAppendSegmentVec {
    pub fn iter(&self) -> OtxAppendSegmentVecIteratorRef<'_> {
        let len = self.len().unwrap();
        OtxAppendSegmentVecIteratorRef {
            cur: &self,
            index: 0,
            len,
        }
    }
}
impl OtxAppendSegmentVec {
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
    pub fn append_segments(&self) -> Result<OtxAppendSegmentVec, Error> {
        let cur = self.cursor.table_slice_by_index(10usize)?;
        Ok(cur.into())
    }
}
impl Otx {
    pub fn base_seals(&self) -> Result<LockSealVec, Error> {
        let cur = self.cursor.table_slice_by_index(11usize)?;
        Ok(cur.into())
    }
}
impl Otx {
    pub fn verify(&self, compatible: bool) -> Result<(), Error> {
        self.cursor.verify_table(12usize, compatible)?;
        self.message()?.verify(compatible)?;
        self.append_segments()?.verify(compatible)?;
        self.base_seals()?.verify(compatible)?;
        Ok(())
    }
}

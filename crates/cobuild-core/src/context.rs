use alloc::vec::Vec;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ScriptHashIndex {
    pub input_locks: Vec<[u8; 32]>,
    pub input_types: Vec<Option<[u8; 32]>>,
    pub output_types: Vec<Option<[u8; 32]>>,
}

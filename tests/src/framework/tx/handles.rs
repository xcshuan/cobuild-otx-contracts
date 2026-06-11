#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct InputHandle(pub(crate) usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct OutputHandle(pub(crate) usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct CellDepHandle(pub(crate) usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct HeaderDepHandle(pub(crate) usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct OtxHandle(pub(crate) usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct WitnessHandle(pub(crate) usize);

pub trait TxEntityHandle: Copy {
    fn from_raw(index: usize) -> Self;
    fn raw(self) -> usize;
}

macro_rules! impl_handle {
    ($handle:ident) => {
        impl $handle {
            pub fn from_raw(index: usize) -> Self {
                Self(index)
            }
        }

        impl TxEntityHandle for $handle {
            fn from_raw(index: usize) -> Self {
                Self(index)
            }

            fn raw(self) -> usize {
                self.0
            }
        }
    };
}

impl_handle!(InputHandle);
impl_handle!(OutputHandle);
impl_handle!(CellDepHandle);
impl_handle!(HeaderDepHandle);
impl_handle!(OtxHandle);
impl_handle!(WitnessHandle);

#[derive(Clone, Debug)]
pub struct EntityIndexMap<T> {
    handle_to_tx_index: Vec<usize>,
    tx_index_to_handle: Vec<Option<T>>,
}

impl<T> Default for EntityIndexMap<T> {
    fn default() -> Self {
        Self {
            handle_to_tx_index: Vec::new(),
            tx_index_to_handle: Vec::new(),
        }
    }
}

impl<T: TxEntityHandle> EntityIndexMap<T> {
    pub fn tx_index(&self, handle: T) -> usize {
        self.handle_to_tx_index[handle.raw()]
    }

    pub fn handle_at_tx_index(&self, index: usize) -> Option<T> {
        self.tx_index_to_handle.get(index).copied().flatten()
    }

    pub(crate) fn insert(&mut self, handle: T, tx_index: usize) {
        let handle_index = handle.raw();
        if self.handle_to_tx_index.len() <= handle_index {
            self.handle_to_tx_index.resize(handle_index + 1, usize::MAX);
        }
        if self.tx_index_to_handle.len() <= tx_index {
            self.tx_index_to_handle.resize(tx_index + 1, None);
        }
        self.handle_to_tx_index[handle_index] = tx_index;
        self.tx_index_to_handle[tx_index] = Some(handle);
    }
}

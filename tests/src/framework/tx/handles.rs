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

macro_rules! impl_handle {
    ($handle:ident) => {
        impl $handle {
            #[allow(dead_code)]
            pub(crate) fn from_raw(index: usize) -> Self {
                Self(index)
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
    handle_to_tx_index: Vec<(T, usize)>,
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

impl<T: Copy + Eq> EntityIndexMap<T> {
    pub fn tx_index(&self, handle: T) -> usize {
        self.handle_to_tx_index
            .iter()
            .find_map(|(indexed_handle, tx_index)| (*indexed_handle == handle).then_some(*tx_index))
            .expect("unknown transaction entity handle")
    }

    pub fn handle_at_tx_index(&self, index: usize) -> Option<T> {
        self.tx_index_to_handle.get(index).copied().flatten()
    }

    pub(crate) fn insert(&mut self, handle: T, tx_index: usize) {
        if self.tx_index_to_handle.len() <= tx_index {
            self.tx_index_to_handle.resize(tx_index + 1, None);
        }
        self.handle_to_tx_index.push((handle, tx_index));
        self.tx_index_to_handle[tx_index] = Some(handle);
    }

    pub(crate) fn set_tx_index(&mut self, handle: T, tx_index: usize) {
        if let Some((_, indexed_tx_index)) = self
            .handle_to_tx_index
            .iter_mut()
            .find(|(indexed_handle, _)| *indexed_handle == handle)
        {
            *indexed_tx_index = tx_index;
        } else {
            self.handle_to_tx_index.push((handle, tx_index));
        }
        self.rebuild_tx_index_to_handle();
    }

    pub(crate) fn remap_tx_indexes(&mut self, mut remap: impl FnMut(usize) -> usize) {
        for (_, tx_index) in &mut self.handle_to_tx_index {
            *tx_index = remap(*tx_index);
        }
        self.rebuild_tx_index_to_handle();
    }

    pub(crate) fn handles(&self) -> impl Iterator<Item = T> + '_ {
        self.handle_to_tx_index.iter().map(|(handle, _)| *handle)
    }

    fn rebuild_tx_index_to_handle(&mut self) {
        let len = self
            .handle_to_tx_index
            .iter()
            .map(|(_, tx_index)| *tx_index)
            .max()
            .map(|max| max + 1)
            .unwrap_or(0);
        self.tx_index_to_handle = vec![None; len];
        for (handle, tx_index) in &self.handle_to_tx_index {
            self.tx_index_to_handle[*tx_index] = Some(*handle);
        }
    }
}

use core::{fmt::Debug, hash::Hash};

pub trait PageId: Default + Hash + Ord + Copy + Debug {}

impl<T: Hash + Ord + Copy + Debug + Default> PageId for T {}

#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct SinglePage;

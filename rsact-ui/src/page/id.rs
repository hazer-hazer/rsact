use core::fmt::Debug;

pub trait PageId: Default + Ord + Copy + Debug {}

impl<T: Ord + Copy + Debug + Default> PageId for T {}

#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct SinglePage;

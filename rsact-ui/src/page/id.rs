use core::fmt::Debug;

pub trait PageId: Ord + Copy + Debug {}

impl<T: Ord + Copy + Debug> PageId for T {}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct SinglePage;

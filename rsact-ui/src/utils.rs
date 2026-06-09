use core::fmt::Display;
use num::PrimInt;

// TODO: Math module

pub fn lerpi<T: PrimInt>(from: T, to: T, factor: T, factor_max: T) -> T {
    // (A*(1024-F) + B * F) >> 10
    // assert!(factor <= factor_max);
    if factor > factor_max {
        return to;
    }
    (from * (factor_max - factor) + to * factor) / factor_max
}

pub struct DisplayTruncated<'a>(&'a str, usize);

impl<'a> Display for DisplayTruncated<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.chars().take(self.1).try_for_each(|c| c.fmt(f))?;

        "...".fmt(f)
    }
}

impl<'a> DisplayTruncated<'a> {
    pub fn new(s: &'a str, max_len: usize) -> Self {
        Self(s, max_len)
    }
}

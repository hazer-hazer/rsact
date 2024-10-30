pub fn cycle_index(index: i64, len: usize) -> usize {
    let index = index % len as i64;
    (if index < 0 { index + len as i64 } else { index }) as usize
}

pub fn min_max_range<T: Ord + Copy>(p1: T, p2: T) -> core::ops::Range<T> {
    let min = core::cmp::min(p1, p2);
    let max = core::cmp::max(p1, p2);

    min..max
}

pub fn min_max_range_incl<T: Ord + Copy>(
    p1: T,
    p2: T,
) -> core::ops::RangeInclusive<T> {
    let min = core::cmp::min(p1, p2);
    let max = core::cmp::max(p1, p2);

    min..=max
}

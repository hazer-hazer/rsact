pub fn cycle_index(index: i64, len: usize) -> usize {
    let index = index % len as i64;
    (if index < 0 { index + len as i64 } else { index }) as usize
}

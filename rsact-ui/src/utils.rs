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

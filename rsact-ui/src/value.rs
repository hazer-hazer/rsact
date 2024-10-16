pub trait RangeValue: Copy + PartialEq {
    fn min() -> Self;
    fn max() -> Self;
    // fn values() -> u64;
    fn real_point(&self) -> f32;
    fn point(&self, len: u32) -> u32;
    fn offset(&self, offset: i32) -> Self;
}

macro_rules! impl_range_value_ints {
    ($($ty:ty),* $(,)?) => {
        $(
            impl RangeValue for $ty {
                fn min() -> Self {
                    Self::MIN
                }

                fn max() -> Self {
                    Self::MAX
                }

                // fn values() -> u64 {
                //     (2 as Self).pow(Self::BITS) as u64
                // }

                fn real_point(&self) -> f32 {
                    *self as f32 / (Self::MAX as f32 - Self::MIN as f32)
                }

                fn point(&self, len: u32) -> u32 {
                    (self.real_point() * len as f32) as u32
                }

                fn offset(&self, offset: i32) -> Self {
                    ((*self as i64 + offset as i64)).clamp(Self::MIN as i64, Self::MAX as i64)as Self
                }
            }
        )*
    };
}

impl_range_value_ints!(u8, u16, u32);

// impl RangeValue for f32 {
//     fn min() -> Self {
//         0.0
//     }

//     fn max() -> Self {
//         1.0
//     }

//     fn real_point(&self) -> f32 {
//         *self
//     }

//     fn point(&self, len: u32) -> u32 {
//         (*self / len as f32) as u32
//     }

//     fn offset(&self, offset: i32) -> Self {

//     }
// }

// TODO: f32/f64
// TODO: Events with offsets must return f32 or other type with known offset properties

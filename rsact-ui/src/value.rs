use core::u8;


pub trait RangeValue: PartialEq + Copy {
    // fn min() -> Self;
    // fn max() -> Self;
    // fn values() -> u64;
    fn real_point(&self) -> f32;
    fn point(&self, len: u32) -> u32;
    fn offset(&self, offset: i32) -> Self;
}

// macro_rules! impl_range_value_ints {
//     ($($ty:ty),* $(,)?) => {
//         $(
//             impl RangeValue for $ty {
//                 fn min() -> Self {
//                     Self::MIN
//                 }

//                 fn max() -> Self {
//                     Self::MAX
//                 }

//                 // fn values() -> u64 {
//                 //     (2 as Self).pow(Self::BITS) as u64
//                 // }

//                 fn real_point(&self) -> f32 {
//                     *self as f32 / (Self::MAX as f32 - Self::MIN as f32)
//                 }

//                 fn point(&self, len: u32) -> u32 {
//                     (self.real_point() * len as f32) as u32
//                 }

//                 fn offset(&self, offset: i32) -> Self {
//                     ((*self as i64 + offset as i64)).clamp(Self::MIN as i64, Self::MAX as i64)as Self
//                 }
//             }
//         )*
//     };
// }

// impl_range_value_ints!(u8, u16, u32);

#[derive(Clone, Copy, PartialEq)]
pub struct RangeU8<
    const MIN: u8 = { u8::MIN },
    const MAX: u8 = { u8::MAX },
    const STEP: u8 = 1,
>(u8);

impl RangeU8<{ u8::MIN }, { u8::MAX }, 1> {
    pub fn new_full_range(value: u8) -> Self {
        Self::new_clamped(value)
    }
}

impl<const MIN: u8, const MAX: u8, const STEP: u8> RangeU8<MIN, MAX, STEP> {
    pub fn new_clamped(value: u8) -> Self {
        Self(value.clamp(MIN, MAX))
    }

    pub fn with_min<const NEW_MIN: u8>(self) -> RangeU8<NEW_MIN, MAX, STEP> {
        RangeU8::new_clamped(self.0)
    }

    pub fn with_max<const NEW_MAX: u8>(self) -> RangeU8<MIN, NEW_MAX, STEP> {
        RangeU8::new_clamped(self.0)
    }

    pub fn with_step<const NEW_STEP: u8>(self) -> RangeU8<MIN, MAX, NEW_STEP> {
        RangeU8::new_clamped(self.0)
    }
}

impl<const MIN: u8, const MAX: u8, const STEP: u8> From<i64>
    for RangeU8<MIN, MAX, STEP>
{
    fn from(value: i64) -> Self {
        Self(value.clamp(MIN as i64, MAX as i64) as u8)
    }
}

impl<const MIN: u8, const MAX: u8, const STEP: u8> RangeValue
    for RangeU8<MIN, MAX, STEP>
{
    // fn min() -> Self {
    //     Self(MIN)
    // }

    // fn max() -> Self {
    //     Self(MAX)
    // }

    fn real_point(&self) -> f32 {
        self.0 as f32 / (MAX as f32 - MIN as f32)
    }

    fn point(&self, len: u32) -> u32 {
        (self.real_point() * len as f32) as u32
    }

    fn offset(&self, offset: i32) -> Self {
        (self.0 as i64 + offset as i64).clamp(MIN as i64, MAX as i64).into()
    }
}

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

//     fn offset(&self, offset: i32) -> Self {}
// }

// TODO: f32/f64
// TODO: Events with offsets must return f32 or other type with known offset properties

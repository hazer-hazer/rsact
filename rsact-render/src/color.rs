use core::fmt::Debug;

pub const ACCENT_COUNT: usize = 6;

pub trait Color: Copy + PartialEq + Debug {
    const WHITE: Self;
    const BLACK: Self;

    fn default_foreground() -> Self;
    fn default_background() -> Self;

    /// Accents are used for internal UI elements. For RGB color it contains
    /// different colors to be used for contrasting element. For colors with
    /// low resolution like BinaryColor or 2-bit color (4 colors only) it is
    /// constrained to never contain same colors subsequently, so for
    /// BinaryColor it is [black, white, black, white, black, white].
    fn accents() -> [Self; ACCENT_COUNT];

    fn map(&self, f: impl Fn(u8) -> u8) -> Self;
    fn fold(&self, other: Self, f: impl Fn(u8, u8) -> u8) -> Self;

    fn from_rgba(rgba: Rgba) -> Self;
    fn into_rgba(&self) -> Rgba;

    fn map_through_rgba<C: Color>(&self) -> C {
        C::from_rgba(self.into_rgba())
    }

    fn invert(&self) -> Self {
        self.map(|c| 255 - c)
    }

    // TODO: Rewrite to use integer math
    // fn mix(&self, alpha: u8, other: Self) -> Self {
    //     // let this_alpha = 1.0 - alpha;
    //     let alpha = alpha as u16;
    //     let this_alpha = 256 - alpha;
    //     self.fold(other, |this, other| {
    //         ((this as u16 * this_alpha + other as u16 * alpha) >> 8) as u8
    //     })
    // }
    fn mix(&self, alpha: f32, other: Self) -> Self {
        let this_alpha = 1.0 - alpha;
        self.fold(other, |this, other| {
            (this as f32 * this_alpha + other as f32 * alpha) as u8
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

pub trait RgbColor: Color {
    fn rgb(r: u8, g: u8, b: u8) -> Self;

    fn r(&self) -> u8;
    fn g(&self) -> u8;
    fn b(&self) -> u8;

    #[inline]
    fn hex(hex: u32) -> Self {
        Self::rgb(
            (hex & 0xff0000 >> 16) as u8,
            (hex & 0x00ff00 >> 8) as u8,
            (hex & 0x0000ff) as u8,
        )
    }

    fn lighten(self, amount: f32) -> Self {
        self.mix(amount, Self::WHITE)
    }

    fn darken(self, amount: f32) -> Self {
        self.mix(amount, Self::BLACK)
    }

    fn dim(self, amount: f32) -> Self {
        self.mix(
            amount,
            Self::from_rgba(Rgba { r: 128, g: 128, b: 128, a: 255 }),
        )
    }

    // #[inline]
    // fn fold(&self, other: Self, f: impl Fn(u8, u8) -> u8) -> Self {
    //     Self::rgb(
    //         f(self.r(), other.r()),
    //         f(self.g(), other.g()),
    //         f(self.b(), other.b()),
    //     )
    // }

    // fn mix(&self, alpha: f32, other: Self) -> Self {
    //     let this_alpha = 1.0 - alpha;
    //     self.fold(other, |this, other| {
    //         (this as f32 * this_alpha + other as f32 * alpha) as u8
    //     })
    // }
}

pub trait RgbaColor: RgbColor {
    fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self;

    fn a(&self) -> u8;
}

pub trait ByteOrder {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BigEndian {}
impl ByteOrder for BigEndian {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LittleEndian {}
impl ByteOrder for LittleEndian {}

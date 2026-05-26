use core::fmt::Debug;

pub trait Color: Copy + PartialEq + Debug {
    fn default_foreground() -> Self;
    fn default_background() -> Self;
    fn accents() -> [Self; 6];
    fn map(&self, f: impl Fn(u8) -> u8) -> Self;
    fn fold(&self, other: Self, f: impl Fn(u8, u8) -> u8) -> Self;

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

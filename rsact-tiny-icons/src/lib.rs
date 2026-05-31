#![no_std]

use core::marker::PhantomData;
use rsact_render::{
    color::{BigEndian, ByteOrder, Color},
    geometry::Point,
    output::pixel::Pixel,
};

mod rendered;
pub use rendered::*;

// TODO: Should constants be private to crate so user is not distracted with
// thousands of constants with the same name for different icon sizes modules?

pub type DefaultByteOrder = BigEndian;

#[derive(Clone, Copy)]
pub struct IconRaw<BO: ByteOrder = DefaultByteOrder> {
    pub data: &'static [u8],
    pub size: u32,
    bo: PhantomData<BO>,
}

impl<BO: ByteOrder> IconRaw<BO> {
    pub const fn new(data: &'static [u8], size: u32) -> Self {
        Self { data, size, bo: PhantomData }
    }

    // TODO: Really use byte order.
    fn bit(&self, x: u32, y: u32) -> bool {
        let index = x + y * self.size;
        (self.data[index as usize / 8] & (0b1000_0000 >> index % 8)) != 0
    }
}

#[derive(Clone, Copy)]
pub struct Icon<C: Color, BO: ByteOrder> {
    raw: IconRaw<BO>,
    position: Point,
    background: Option<C>,
    foreground: Option<C>,
}

impl<C: Color, BO: ByteOrder> Icon<C, BO> {
    pub fn new(
        raw: IconRaw<BO>,
        position: Point,
        background: Option<C>,
        foreground: Option<C>,
    ) -> Self {
        Self { raw, position, background, foreground }
    }

    pub fn iter(&self) -> impl Iterator<Item = Pixel<C>> + '_ {
        (0..self.raw.size)
            .map(move |y| {
                (0..self.raw.size).map(move |x| {
                    self.color(self.raw.bit(x, y)).map(|color| {
                        Pixel(
                            Point::new(x as i32, y as i32) + self.position,
                            color,
                        )
                    })
                })
            })
            .flatten()
            .filter_map(|pixel| pixel)
    }

    fn color(&self, bit: bool) -> Option<C> {
        if bit { self.foreground } else { self.background }
    }
}

pub trait IconSet<BO: ByteOrder = DefaultByteOrder>:
    PartialEq + Sized + 'static
{
    const KINDS: &[Self];

    const SIZES: &[u32];

    fn size(&self, size: u32) -> crate::IconRaw<BO>;
}

// TODO: Real Endianness

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmptyIconSet;

impl IconSet<DefaultByteOrder> for EmptyIconSet {
    const KINDS: &[Self] = &[];

    const SIZES: &[u32] = &[];

    fn size(&self, _size: u32) -> crate::IconRaw<DefaultByteOrder> {
        panic!("Cannot use empty icon set")
    }
}

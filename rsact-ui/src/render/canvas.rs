use super::color::Color;
use crate::prelude::Size;
use alloc::boxed::Box;

trait PackedColor {
    type Storage;

    fn none() -> Self::Storage;
    fn some(inner: Color) -> Self::Storage;
}

pub struct Canvas<C: PackedColor> {
    size: Size,
    pixels: Box<[PackedColor]>,
}

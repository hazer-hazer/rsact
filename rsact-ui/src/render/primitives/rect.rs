use embedded_graphics::prelude::Point;

use crate::prelude::Size;

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    top_left: Point,
    size: Size,
}

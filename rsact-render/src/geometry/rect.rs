use crate::geometry::{
    anchor::{AnchorPoint, AnchorX, AnchorY},
    axis::{Anchor, Axis},
    point::Point, size::Size,
};

/// First-class 2D axis-aligned rectangle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub top_left: Point,
    pub size: Size,
}

impl Rect {
    pub const fn new(top_left: Point, size: Size) -> Self {
        Self { top_left, size }
    }

    pub fn zero() -> Self {
        Self { top_left: Point::zero(), size: Size::zero() }
    }

    pub fn center(&self) -> Point {
        Point::new(
            self.top_left.x + self.size.width as i32 / 2,
            self.top_left.y + self.size.height as i32 / 2,
        )
    }

    pub fn bottom_right(&self) -> Option<Point> {
        if self.size.width == 0 || self.size.height == 0 {
            None
        } else {
            Some(Point::new(
                self.top_left.x + self.size.width as i32 - 1,
                self.top_left.y + self.size.height as i32 - 1,
            ))
        }
    }

    pub fn translate(&self, by: Point) -> Self {
        Self::new(
            Point::new(self.top_left.x + by.x, self.top_left.y + by.y),
            self.size,
        )
    }

    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.top_left.x
            && point.y >= self.top_left.y
            && point.x < self.top_left.x + self.size.width as i32
            && point.y < self.top_left.y + self.size.height as i32
    }

    pub fn intersection(&self, other: &Self) -> Self {
        let x1 = self.top_left.x.max(other.top_left.x);
        let y1 = self.top_left.y.max(other.top_left.y);
        let x2 = (self.top_left.x + self.size.width as i32)
            .min(other.top_left.x + other.size.width as i32);
        let y2 = (self.top_left.y + self.size.height as i32)
            .min(other.top_left.y + other.size.height as i32);
        if x2 > x1 && y2 > y1 {
            Self::new(
                Point::new(x1, y1),
                Size::new((x2 - x1) as u32, (y2 - y1) as u32),
            )
        } else {
            Self::zero()
        }
    }

    pub fn resized_width(&self, new_width: u32, anchor: AnchorX) -> Self {
        let dx = new_width as i32 - self.size.width as i32;
        let new_x = match anchor {
            AnchorX::Left => self.top_left.x,
            AnchorX::Center => self.top_left.x - dx / 2,
            AnchorX::Right => self.top_left.x - dx,
        };
        Self::new(
            Point::new(new_x, self.top_left.y),
            Size::new(new_width, self.size.height),
        )
    }

    pub fn resized_height(&self, new_height: u32, anchor: AnchorY) -> Self {
        let dy = new_height as i32 - self.size.height as i32;
        let new_y = match anchor {
            AnchorY::Top => self.top_left.y,
            AnchorY::Center => self.top_left.y - dy / 2,
            AnchorY::Bottom => self.top_left.y - dy,
        };
        Self::new(
            Point::new(self.top_left.x, new_y),
            Size::new(self.size.width, new_height),
        )
    }

    pub fn translate_mut(&mut self, by: Point) -> &mut Self {
        self.top_left = self.top_left + by;
        self
    }

    /// Return the point corresponding to the given anchor within this rect.
    pub fn anchor_point(&self, anchor: AnchorPoint) -> Point {
        let w = self.size.width as i32;
        let h = self.size.height as i32;
        let half_w = w / 2;
        let half_h = h / 2;
        let x = match anchor {
            AnchorPoint::TopLeft
            | AnchorPoint::CenterLeft
            | AnchorPoint::BottomLeft => self.top_left.x,
            AnchorPoint::TopCenter
            | AnchorPoint::Center
            | AnchorPoint::BottomCenter => self.top_left.x + half_w,
            AnchorPoint::TopRight
            | AnchorPoint::CenterRight
            | AnchorPoint::BottomRight => self.top_left.x + w - 1,
        };
        let y = match anchor {
            AnchorPoint::TopLeft
            | AnchorPoint::TopCenter
            | AnchorPoint::TopRight => self.top_left.y,
            AnchorPoint::CenterLeft
            | AnchorPoint::Center
            | AnchorPoint::CenterRight => self.top_left.y + half_h,
            AnchorPoint::BottomLeft
            | AnchorPoint::BottomCenter
            | AnchorPoint::BottomRight => self.top_left.y + h - 1,
        };
        Point::new(x, y)
    }
}

#[cfg(feature = "embedded-graphics")]
impl From<embedded_graphics::primitives::Rectangle> for Rect {
    fn from(r: embedded_graphics::primitives::Rectangle) -> Self {
        Self::new(r.top_left.into(), r.size.into())
    }
}

#[cfg(feature = "embedded-graphics")]
impl From<Rect> for embedded_graphics::primitives::Rectangle {
    fn from(r: Rect) -> Self {
        embedded_graphics::primitives::Rectangle::new(
            r.top_left.into(),
            r.size.into(),
        )
    }
}

pub trait RectExt {
    fn center_offset_of(&self, child: Self) -> Point;
    fn resized_axis(&self, axis: Axis, size: u32, anchor: Anchor) -> Self;
}

impl RectExt for Rect {
    fn center_offset_of(&self, child: Self) -> Point {
        self.center() - child.center()
    }

    fn resized_axis(&self, axis: Axis, value: u32, anchor: Anchor) -> Self {
        match axis {
            Axis::X => self.resized_width(value, anchor.into()),
            Axis::Y => self.resized_height(value, anchor.into()),
        }
    }
}

use crate::{
    geometry::{
        anchor::{AnchorPoint, AnchorX, AnchorY},
        axis::{Anchor, Axis},
        padding::Padding,
        point::Point,
        size::Size,
    },
    primitives::Primitive,
};
use core::fmt::Display;

/// First-class 2D axis-aligned rectangle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub top_left: Point,
    pub size: Size,
}

impl Display for Rect {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "[{}; {}]", self.top_left, self.size)
    }
}

impl Rect {
    pub const fn new(top_left: Point, size: Size) -> Self {
        Self { top_left, size }
    }

    pub const fn top_left(size: Size) -> Self {
        Self { top_left: Point::new(0, 0), size }
    }

    pub const fn zero() -> Self {
        Self { top_left: Point::zero(), size: Size::zero() }
    }

    pub const fn is_zero_sized(&self) -> bool {
        self.size.is_zero_area()
    }

    pub fn columns(&self) -> core::ops::Range<i32> {
        // TODO: EG-like SaturatingAs
        self.top_left.x..self.top_left.x.saturating_add(self.size.width as i32)
    }

    pub fn rows(&self) -> core::ops::Range<i32> {
        // TODO: EG-like SaturatingAs
        self.top_left.y..self.top_left.y.saturating_add(self.size.height as i32)
    }

    pub fn points(&self) -> Points {
        Points::new(self)
    }

    pub fn center(&self) -> Point {
        Point::new(
            self.top_left.x + self.size.width as i32 / 2,
            self.top_left.y + self.size.height as i32 / 2,
        )
    }

    pub fn bottom_right(&self) -> Option<Point> {
        if self.is_zero_sized() {
            None
        } else {
            Some(Point::new(
                self.top_left.x + self.size.width as i32 - 1,
                self.top_left.y + self.size.height as i32 - 1,
            ))
        }
    }

    pub fn translate_mut(&mut self, by: Point) -> &mut Self {
        self.top_left = self.top_left + by;
        self
    }

    pub fn translate(&self, by: Point) -> Self {
        Self::new(self.top_left + by, self.size)
    }

    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.top_left.x
            && point.y >= self.top_left.y
            && point.x < self.top_left.x + self.size.width as i32
            && point.y < self.top_left.y + self.size.height as i32
    }

    /// The smallest rectangle containing both `self` and `other` — the "damage"
    /// of a box that moved (erase where it was, paint where it is). A
    /// zero-sized rect has no area, so it contributes nothing: `union` with it
    /// returns the other rect (this is what makes a box appearing/disappearing
    /// damage exactly its non-zero rect).
    pub fn union(&self, other: &Self) -> Self {
        if self.is_zero_sized() {
            return *other;
        }
        if other.is_zero_sized() {
            return *self;
        }
        let x1 = self.top_left.x.min(other.top_left.x);
        let y1 = self.top_left.y.min(other.top_left.y);
        // Bottom-right edges are exclusive (`top_left + size`).
        let x2 = (self.top_left.x + self.size.width as i32)
            .max(other.top_left.x + other.size.width as i32);
        let y2 = (self.top_left.y + self.size.height as i32)
            .max(other.top_left.y + other.size.height as i32);
        Self::new(
            Point::new(x1, y1),
            Size::new((x2 - x1) as u32, (y2 - y1) as u32),
        )
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

    /// Whether the two rects share at least one pixel.
    ///
    /// A zero-sized rect covers no pixel, so it intersects nothing — the same
    /// convention [`Self::intersection`] already uses for the disjoint case (it
    /// returns [`Self::zero`], not an `Option`). WS6.4a leans on that: an op with
    /// a zero-area bound paints nothing, so no tile is obliged to draw it.
    pub fn intersects(&self, other: &Self) -> bool {
        !self.intersection(other).is_zero_sized()
    }

    /// Grow this rect outward by `by` on each side.
    ///
    /// WS6.4c(G): the arithmetic behind `paint_bounds` — a widget's painted area
    /// is its layout rect grown by however far it draws *outside* that rect
    /// (`ext_draw`: an outline today, box shadows and tooltips later). The
    /// inverse of shrinking by padding, hence the name.
    ///
    /// Saturating on both axes: the top-left cannot wrap past `i32::MIN` and the
    /// size cannot wrap past `u32::MAX`. Growing is the *safe* direction for
    /// every consumer — a bound that is too large costs redundant paint, a bound
    /// that is too small drops it — so saturation degrades toward correctness.
    pub fn outset(&self, by: Padding) -> Self {
        Self {
            top_left: Point::new(
                self.top_left.x.saturating_sub(by.left as i32),
                self.top_left.y.saturating_sub(by.top as i32),
            ),
            size: Size::new(
                self.size.width.saturating_add(by.left + by.right),
                self.size.height.saturating_add(by.top + by.bottom),
            ),
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

    pub fn resized_center(&self, new_size: Size) -> Self {
        self.resized_width(new_size.width, AnchorX::Center)
            .resized_height(new_size.height, AnchorY::Center)
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

impl Primitive for Rect {
    fn into_kind(self) -> crate::prelude::PrimitiveKind {
        crate::prelude::PrimitiveKind::Rect(self)
    }

    fn translate_mut(&mut self, by: Point) -> &mut Self {
        self.top_left = self.top_left + by;
        self
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

pub struct Points {
    x: core::ops::Range<i32>,
    y: core::ops::Range<i32>,
    x_start: i32,
}

impl Iterator for Points {
    type Item = Point;

    fn next(&mut self) -> Option<Self::Item> {
        while !self.y.is_empty() {
            if let Some(x) = self.x.next() {
                return Some(Point::new(x, self.y.start));
            }

            self.y.next();
            self.x.start = self.x_start;
        }

        None
    }
}

impl Points {
    pub const fn empty() -> Self {
        Self { x: 0..0, y: 0..0, x_start: 0 }
    }

    fn new(rect: &Rect) -> Self {
        if rect.is_zero_sized() {
            return Self::empty();
        }

        let x = rect.columns();
        let y = rect.rows();
        let x_start = x.start;

        Self { x, y, x_start }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

pub trait Sided<T> {
    fn side(&self, side: Side) -> T;
}

impl Sided<u32> for Rect {
    fn side(&self, side: Side) -> u32 {
        match side {
            Side::Top | Side::Bottom => self.size.width,
            Side::Left | Side::Right => self.size.height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Rect;
    use crate::geometry::{point::Point, size::Size};

    fn r(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect::new(Point::new(x, y), Size::new(w, h))
    }

    #[test]
    fn union_of_disjoint_rects_is_their_bounding_box() {
        // Two 10x10 boxes, one at origin, one at (20,20): the bounding box spans
        // (0,0)..(30,30).
        assert_eq!(r(0, 0, 10, 10).union(&r(20, 20, 10, 10)), r(0, 0, 30, 30));
    }

    #[test]
    fn union_is_commutative_and_covers_a_shift() {
        // A box that moved right by 5: damage = old ∪ new = (0,0)..(15,10).
        let old = r(0, 0, 10, 10);
        let new = r(5, 0, 10, 10);
        assert_eq!(old.union(&new), r(0, 0, 15, 10));
        assert_eq!(new.union(&old), r(0, 0, 15, 10));
    }

    #[test]
    fn union_with_zero_sized_contributes_nothing() {
        let sized = r(3, 4, 10, 10);
        // A box appearing (was zero) or disappearing (now zero) damages exactly
        // its non-zero rect, not a huge box back to the origin.
        assert_eq!(Rect::zero().union(&sized), sized);
        assert_eq!(sized.union(&Rect::zero()), sized);
        assert_eq!(Rect::zero().union(&Rect::zero()), Rect::zero());
    }

    /// WS6.4a: `intersects` decides which tiles must redraw an op, so the
    /// touching-but-not-overlapping boundary is the case that matters — the
    /// bottom-right edge is exclusive, so two rects sharing an edge do NOT
    /// intersect. Off by one here and every tile boundary either double-paints or
    /// cracks.
    #[test]
    fn intersects_treats_the_shared_edge_as_disjoint() {
        let left = r(0, 0, 10, 10);
        assert!(left.intersects(&r(9, 0, 10, 10)), "one shared column");
        assert!(!left.intersects(&r(10, 0, 10, 10)), "abutting, no overlap");
        assert!(!left.intersects(&r(0, 10, 10, 10)), "abutting below");
        assert!(left.intersects(&left));
        // A zero-sized rect covers no pixel, so it meets nothing — not even a
        // rect that contains its corner.
        assert!(!left.intersects(&Rect::zero()));
        assert!(!Rect::zero().intersects(&Rect::zero()));
    }
}

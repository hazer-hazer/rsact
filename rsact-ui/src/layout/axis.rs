use core::ops::Add;

use embedded_graphics::geometry::{AnchorPoint, AnchorX, AnchorY, Point};

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(::defmt::Format))]
pub enum Axis {
    X,
    Y,
}

impl Axis {
    pub fn axial<T: Axial>(self, data: T) -> AxialData<T> {
        AxialData { axis: self, data }
    }

    pub fn canon<T: Axial>(self, main: T::Data, cross: T::Data) -> T {
        match self {
            Axis::X => T::axial_new(main, cross),
            Axis::Y => T::axial_new(cross, main),
        }
    }

    pub const fn length_name(&self) -> &str {
        match self {
            Axis::X => "width",
            Axis::Y => "height",
        }
    }

    pub const fn dir_name(&self) -> &str {
        match self {
            Axis::X => "row",
            Axis::Y => "col",
        }
    }

    // Apply some infix operation (e.g. operator) on two axial structures
    pub fn infix<T, M, C>(self, lhs: T, rhs: T, main: M, cross: C) -> T
    where
        T: Axial,
        M: Fn(T::Data, T::Data) -> T::Data,
        C: Fn(T::Data, T::Data) -> T::Data,
    {
        self.canon(
            main(lhs.main(self), rhs.main(self)),
            cross(lhs.cross(self), rhs.cross(self)),
        )
    }

    pub fn inverted(self) -> Self {
        match self {
            Axis::X => Axis::Y,
            Axis::Y => Axis::X,
        }
    }
}

pub trait Axial {
    type Data;

    fn x(&self) -> Self::Data;
    fn y(&self) -> Self::Data;
    fn x_mut(&mut self) -> &mut Self::Data;
    fn y_mut(&mut self) -> &mut Self::Data;
    fn axial_new(x: Self::Data, y: Self::Data) -> Self;

    fn axial_map<F, U>(&self, f: F) -> (U, U)
    where
        Self: Sized,
        F: Fn(Self::Data) -> U,
    {
        (f(self.x()), f(self.y()))
    }

    fn destruct(&self) -> (Self::Data, Self::Data) {
        (self.x(), self.y())
    }

    #[inline]
    fn main(&self, axis: Axis) -> Self::Data {
        match axis {
            Axis::X => self.x(),
            Axis::Y => self.y(),
        }
    }

    #[inline]
    fn cross(&self, axis: Axis) -> Self::Data {
        match axis {
            Axis::X => self.y(),
            Axis::Y => self.x(),
        }
    }

    #[inline]
    fn main_mut(&mut self, axis: Axis) -> &mut Self::Data {
        match axis {
            Axis::X => self.x_mut(),
            Axis::Y => self.y_mut(),
        }
    }

    #[inline]
    fn cross_mut(&mut self, axis: Axis) -> &mut Self::Data {
        match axis {
            Axis::X => self.y_mut(),
            Axis::Y => self.x_mut(),
        }
    }

    #[inline]
    fn into_axial(self, axis: Axis) -> AxialData<Self>
    where
        Self: Sized,
    {
        AxialData { axis, data: self }
    }

    #[inline]
    fn add_main(self, axis: Axis, main: Self::Data) -> Self
    where
        Self::Data: Add<Self::Data, Output = Self::Data>,
        Self: Sized + Copy,
    {
        self.with_main(axis, self.main(axis) + main)
    }

    #[inline]
    fn add_cross(self, axis: Axis, cross: Self::Data) -> Self
    where
        Self::Data: Add<Self::Data, Output = Self::Data>,
        Self: Sized + Copy,
    {
        self.with_cross(axis, self.cross(axis) + cross)
    }

    #[inline]
    fn with_main(self, axis: Axis, main: Self::Data) -> Self
    where
        Self: Sized,
    {
        axis.canon(main, self.cross(axis))
    }

    #[inline]
    fn with_cross(self, axis: Axis, cross: Self::Data) -> Self
    where
        Self: Sized,
    {
        axis.canon(self.main(axis), cross)
    }

    // fn main_sum<O>(&self, rhs: impl Axial<O>) -> Self
    // where
    //     T: core::ops::Add<Output = T>,
    //     O: Into<T>,
    //     Self: Sized,
    // {
    //     Self::canon_new(self.main() + rhs.main().into(), self.cross())
    // }

    // fn cross_sum<O>(&self, rhs: impl Axial<O>) -> Self
    // where
    //     T: core::ops::Add<Output = T>,
    //     O: Into<T>,
    //     Self: Sized,
    // {
    //     Self::canon_new(self.main(), self.cross() + rhs.cross().into())
    // }
}

impl Axial for Point {
    type Data = i32;

    fn x(&self) -> Self::Data {
        self.x
    }

    fn y(&self) -> Self::Data {
        self.y
    }

    fn x_mut(&mut self) -> &mut Self::Data {
        &mut self.x
    }

    fn y_mut(&mut self) -> &mut Self::Data {
        &mut self.y
    }

    #[inline]
    fn axial_new(x: Self::Data, y: Self::Data) -> Self {
        Self::new(x, y)
    }
}

impl Axial for embedded_graphics_core::geometry::Size {
    type Data = u32;

    #[inline]
    fn x(&self) -> Self::Data {
        self.width
    }

    #[inline]
    fn y(&self) -> Self::Data {
        self.height
    }

    fn x_mut(&mut self) -> &mut Self::Data {
        &mut self.width
    }

    fn y_mut(&mut self) -> &mut Self::Data {
        &mut self.height
    }

    #[inline]
    fn axial_new(x: Self::Data, y: Self::Data) -> Self {
        Self::new(x, y)
    }
}

impl<T: Copy> Axial for (T, T) {
    type Data = T;

    #[inline]
    fn x(&self) -> Self::Data {
        self.0
    }

    #[inline]
    fn y(&self) -> Self::Data {
        self.1
    }

    fn x_mut(&mut self) -> &mut Self::Data {
        &mut self.0
    }

    fn y_mut(&mut self) -> &mut Self::Data {
        &mut self.1
    }

    #[inline]
    fn axial_new(x: Self::Data, y: Self::Data) -> Self {
        (x, y)
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(::defmt::Format))]
pub struct AxialData<T: Axial> {
    axis: Axis,
    data: T,
}

impl<T: Axial> AxialData<T> {
    #[inline]
    pub fn main(&self) -> T::Data {
        match self.axis {
            Axis::X => self.data.x(),
            Axis::Y => self.data.y(),
        }
    }

    #[inline]
    pub fn cross(&self) -> T::Data {
        match self.axis {
            Axis::X => self.data.y(),
            Axis::Y => self.data.x(),
        }
    }
}

pub trait Direction {
    const AXIS: Axis;
}

pub struct RowDir;
impl Direction for RowDir {
    const AXIS: Axis = Axis::X;
}

pub struct ColDir;
impl Direction for ColDir {
    const AXIS: Axis = Axis::Y;
}

#[derive(Clone, Copy)]
pub enum Anchor {
    Start,
    Center,
    End,
}

impl Into<AnchorX> for Anchor {
    fn into(self) -> AnchorX {
        match self {
            Anchor::Start => AnchorX::Left,
            Anchor::Center => AnchorX::Center,
            Anchor::End => AnchorX::Right,
        }
    }
}

impl Into<AnchorY> for Anchor {
    fn into(self) -> AnchorY {
        match self {
            Anchor::Start => embedded_graphics::geometry::AnchorY::Top,
            Anchor::Center => embedded_graphics::geometry::AnchorY::Center,
            Anchor::End => embedded_graphics::geometry::AnchorY::Bottom,
        }
    }
}

#[derive(Clone, Copy)]
pub struct AxisAnchorPoint {
    x: Anchor,
    y: Anchor,
}

impl Into<AnchorPoint> for AxisAnchorPoint {
    fn into(self) -> AnchorPoint {
        AnchorPoint::from_xy(self.x.into(), self.y.into())
    }
}

impl Axial for AxisAnchorPoint {
    type Data = Anchor;

    fn x(&self) -> Self::Data {
        self.x
    }

    fn y(&self) -> Self::Data {
        self.y
    }

    fn x_mut(&mut self) -> &mut Self::Data {
        &mut self.x
    }

    fn y_mut(&mut self) -> &mut Self::Data {
        &mut self.y
    }

    fn axial_new(x: Self::Data, y: Self::Data) -> Self {
        Self { x, y }
    }
}

#[cfg(test)]
mod tests {
    use embedded_graphics::geometry::Point;

    use super::{Axial, Axis};

    #[test]
    fn x() {
        let point = Point::new(100, 500);
        let axial = point.into_axial(Axis::X);
        assert_eq!(axial.main(), 100);
        assert_eq!(axial.cross(), 500);
    }

    #[test]
    fn y() {
        let point = Point::new(100, 500);
        let axial = point.into_axial(Axis::Y);
        assert_eq!(axial.main(), 500);
        assert_eq!(axial.cross(), 100);
    }
}

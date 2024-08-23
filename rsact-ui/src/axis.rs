use embedded_graphics::geometry::Point;

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(::defmt::Format))]
pub enum Axis {
    X,
    Y,
}

impl Axis {
    pub fn axial<T: Axial>(&self, data: T) -> AxialData<T> {
        AxialData { axis: *self, data }
    }

    pub fn canon<T: Axial>(&self, main: T::Data, cross: T::Data) -> T {
        match self {
            Axis::X => T::new(main, cross),
            Axis::Y => T::new(cross, main),
        }
    }

    pub fn invert(self) -> Self {
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
    fn new(x: Self::Data, y: Self::Data) -> Self;

    #[inline]
    fn main_for(&self, axis: Axis) -> Self::Data {
        match axis {
            Axis::X => self.x(),
            Axis::Y => self.y(),
        }
    }

    #[inline]
    fn cross_for(&self, axis: Axis) -> Self::Data {
        match axis {
            Axis::X => self.y(),
            Axis::Y => self.x(),
        }
    }

    #[inline]
    fn into_axial(self, axis: Axis) -> AxialData<Self>
    where
        Self: Sized,
    {
        AxialData { axis, data: self }
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

    #[inline]
    fn x(&self) -> Self::Data {
        self.x
    }

    #[inline]
    fn y(&self) -> Self::Data {
        self.y
    }

    #[inline]
    fn new(x: Self::Data, y: Self::Data) -> Self {
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

    #[inline]
    fn new(x: Self::Data, y: Self::Data) -> Self {
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

    #[inline]
    fn new(x: Self::Data, y: Self::Data) -> Self {
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

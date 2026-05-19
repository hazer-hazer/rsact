use core::ops::{Add, Neg, Sub};

/// First-class angular measurement.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Angle {
    pub radians: f32,
}

impl Angle {
    pub fn zero() -> Self {
        Self { radians: 0.0 }
    }

    pub fn from_degrees(degrees: f32) -> Self {
        Self { radians: degrees * core::f32::consts::PI / 180.0 }
    }

    pub fn from_radians(radians: f32) -> Self {
        Self { radians }
    }

    pub fn to_degrees(self) -> f32 {
        self.radians * 180.0 / core::f32::consts::PI
    }

    pub fn to_radians(self) -> f32 {
        self.radians
    }
}

impl Add for Angle {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self { radians: self.radians + rhs.radians }
    }
}

impl Sub for Angle {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self { radians: self.radians - rhs.radians }
    }
}

impl Neg for Angle {
    type Output = Self;
    fn neg(self) -> Self {
        Self { radians: -self.radians }
    }
}

#[cfg(feature = "embedded-graphics")]
impl From<embedded_graphics::geometry::Angle> for Angle {
    fn from(a: embedded_graphics::geometry::Angle) -> Self {
        Self::from_degrees(a.to_degrees())
    }
}

#[cfg(feature = "embedded-graphics")]
impl From<Angle> for embedded_graphics::geometry::Angle {
    fn from(a: Angle) -> Self {
        Self::from_degrees(a.to_degrees())
    }
}

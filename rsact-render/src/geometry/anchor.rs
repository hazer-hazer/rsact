#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnchorX {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnchorY {
    Top,
    Center,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnchorPoint {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl AnchorPoint {
    pub fn from_xy(x: AnchorX, y: AnchorY) -> Self {
        match (x, y) {
            (AnchorX::Left, AnchorY::Top) => Self::TopLeft,
            (AnchorX::Center, AnchorY::Top) => Self::TopCenter,
            (AnchorX::Right, AnchorY::Top) => Self::TopRight,
            (AnchorX::Left, AnchorY::Center) => Self::CenterLeft,
            (AnchorX::Center, AnchorY::Center) => Self::Center,
            (AnchorX::Right, AnchorY::Center) => Self::CenterRight,
            (AnchorX::Left, AnchorY::Bottom) => Self::BottomLeft,
            (AnchorX::Center, AnchorY::Bottom) => Self::BottomCenter,
            (AnchorX::Right, AnchorY::Bottom) => Self::BottomRight,
        }
    }
}

#[cfg(feature = "embedded-graphics")]
impl From<AnchorPoint> for embedded_graphics::geometry::AnchorPoint {
    fn from(a: AnchorPoint) -> Self {
        match a {
            AnchorPoint::TopLeft => Self::TopLeft,
            AnchorPoint::TopCenter => Self::TopCenter,
            AnchorPoint::TopRight => Self::TopRight,
            AnchorPoint::CenterLeft => Self::CenterLeft,
            AnchorPoint::Center => Self::Center,
            AnchorPoint::CenterRight => Self::CenterRight,
            AnchorPoint::BottomLeft => Self::BottomLeft,
            AnchorPoint::BottomCenter => Self::BottomCenter,
            AnchorPoint::BottomRight => Self::BottomRight,
        }
    }
}

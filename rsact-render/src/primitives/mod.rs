pub mod arc;
pub mod circle;
pub mod ellipse;
pub mod line;
pub mod polygon;
pub mod rounded_rect;
pub mod sector;
pub mod block;

#[derive(Clone, PartialEq, Debug)]
pub enum Primitive {
    Arc(arc::Arc),
    Circle(circle::Circle),
    Ellipse(ellipse::Ellipse),
    Line(line::Line),
    Polygon(polygon::Polygon),
    Rect(crate::geometry::Rect),
    RoundedRect(rounded_rect::RoundedRect),
    Sector(sector::Sector),
}

macro_rules! impl_into_primitive {
    ($($t:ty => $variant:ident),*) => {
        $(
            impl From<$t> for Primitive {
                fn from(value: $t) -> Self {
                    Self::$variant(value)
                }
            }
        )*
    };
}

impl_into_primitive!(
    arc::Arc => Arc,
    circle::Circle => Circle,
    ellipse::Ellipse => Ellipse,
    line::Line => Line,
    polygon::Polygon => Polygon,
    crate::geometry::Rect => Rect,
    rounded_rect::RoundedRect => RoundedRect,
    sector::Sector => Sector
);

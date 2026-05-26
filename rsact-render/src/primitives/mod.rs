use crate::geometry::Point;

pub mod arc;
pub mod block;
pub mod circle;
pub mod ellipse;
pub mod line;
pub mod polygon;
pub mod rounded_rect;
pub mod sector;

#[derive(Clone, PartialEq, Debug)]
pub enum PrimitiveKind {
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
            impl From<$t> for PrimitiveKind {
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

pub trait Primitive {
    fn into_kind(self) -> PrimitiveKind;

    // TODO: Bounding box

    fn translate_mut(&mut self, by: Point) -> &mut Self;

    fn translated(&self, by: Point) -> Self
    where
        Self: Sized + Clone,
    {
        let mut new = self.clone();
        new.translate_mut(by);
        new
    }
}

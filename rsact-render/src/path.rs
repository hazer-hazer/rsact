use crate::geometry::*;
use alloc::vec::Vec;

/// A single segment of a path.
#[derive(Clone, Debug, PartialEq)]
pub enum PathSegment {
    MoveTo(Point),
    LineTo(Point),
    ArcTo { center: Point, radius: u32, start: Angle, sweep: Angle },
    Close,
}

/// An immutable path composed of line/arc segments.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    pub(crate) segments: Vec<PathSegment>,
}

impl Path {
    pub fn segments(&self) -> &[PathSegment] {
        &self.segments
    }
}

impl From<PathBuilder> for Path {
    fn from(builder: PathBuilder) -> Self {
        builder.build()
    }
}

/// Builder for constructing a [`Path`].
#[derive(Clone, Debug, Default)]
pub struct PathBuilder {
    segments: Vec<PathSegment>,
}

impl PathBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn move_to(mut self, point: Point) -> Self {
        self.segments.push(PathSegment::MoveTo(point));
        self
    }

    pub fn line_to(mut self, point: Point) -> Self {
        self.segments.push(PathSegment::LineTo(point));
        self
    }

    pub fn arc_to(
        mut self,
        center: Point,
        radius: u32,
        start: Angle,
        sweep: Angle,
    ) -> Self {
        self.segments.push(PathSegment::ArcTo { center, radius, start, sweep });
        self
    }

    pub fn close(mut self) -> Self {
        self.segments.push(PathSegment::Close);
        self
    }

    pub fn build(self) -> Path {
        Path { segments: self.segments }
    }
}

use crate::geometry::*;
use alloc::vec::Vec;

/// A single segment of a path.
#[derive(Clone, Debug, PartialEq)]
pub enum PathSegment {
    MoveTo(Point),
    LineTo(Point),
    // TODO: If we get better at our own arc drawing we should support full
    // SVG-like arc functionality.
    ArcTo { center: Point, radius: u32, start: Angle, sweep: Angle },
    Close,
}

/// An immutable path composed of segments.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    pub segments: Vec<PathSegment>,
}

impl Path {
    pub fn segments(&self) -> &[PathSegment] {
        &self.segments
    }

    /// The smallest axis-aligned rect *known* to contain every pixel this path
    /// can reach, or `None` for a path that reaches no point at all (empty, or
    /// only [`PathSegment::Close`]).
    ///
    /// Deliberately **conservative**: an [`PathSegment::ArcTo`] contributes its
    /// whole circle (`center ± radius`), not the swept sub-arc. This bound
    /// decides which tiles are *obliged* to redraw an op, and too large only
    /// costs redundant paint where too small lets a culler drop a visible one.
    ///
    /// Stroke width is not part of a path's geometry, so a stroked path paints
    /// up to half a stroke outside this bound — same caveat as
    /// [`DrawOp::bounds`].
    ///
    /// [`DrawOp::bounds`]: crate::record::DrawOp::bounds
    pub fn bounds(&self) -> Option<Rect> {
        // Running inclusive (min, max) corner pair.
        let mut bounds: Option<(Point, Point)> = None;
        // Takes the accumulator as an argument rather than capturing it, so the
        // closure stays a plain `Fn` and the `for` loop below can still borrow
        // `bounds` mutably per call.
        let include = |point: Point, at: &mut Option<(Point, Point)>| match at {
            Some((min, max)) => {
                min.x = min.x.min(point.x);
                min.y = min.y.min(point.y);
                max.x = max.x.max(point.x);
                max.y = max.y.max(point.y);
            },
            None => *at = Some((point, point)),
        };

        for segment in &self.segments {
            match *segment {
                PathSegment::MoveTo(point) | PathSegment::LineTo(point) => {
                    include(point, &mut bounds)
                },
                PathSegment::ArcTo { center, radius, .. } => {
                    let radius = radius as i32;
                    include(
                        Point::new(center.x - radius, center.y - radius),
                        &mut bounds,
                    );
                    include(
                        Point::new(center.x + radius, center.y + radius),
                        &mut bounds,
                    );
                },
                // Closing draws back to the subpath start, which is already a
                // `MoveTo`/`LineTo` point — no new extremum.
                PathSegment::Close => {},
            }
        }

        bounds.map(|(min, max)| {
            // Both corners are *inclusive* pixels while `Rect`'s bottom-right
            // edge is exclusive — hence `+ 1`. A single-point path is 1x1, not
            // 0x0, and a zero-sized rect would intersect nothing.
            Rect::new(
                min,
                Size::new(
                    (max.x - min.x + 1) as u32,
                    (max.y - min.y + 1) as u32,
                ),
            )
        })
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

    pub fn with_lines(mut self, points: impl Iterator<Item = Point>) -> Self {
        for point in points {
            if self.segments.is_empty() {
                self.segments.push(PathSegment::MoveTo(point));
            } else {
                self.segments.push(PathSegment::LineTo(point));
            }
        }
        self
    }

    pub fn arc_to(
        mut self,
        center: Point,
        radius: u32,
        start: Angle,
        sweep: Angle,
    ) -> Self {
        self.segments
            .push(PathSegment::ArcTo { center, radius, start, sweep });
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

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect::new(Point::new(x, y), Size::new(w, h))
    }

    #[test]
    fn bounds_span_every_line_point_inclusively() {
        let path = PathBuilder::new()
            .move_to(Point::new(4, 10))
            .line_to(Point::new(9, 2))
            .line_to(Point::new(1, 6))
            .close()
            .build();
        // x: 1..=9 → 9 wide, y: 2..=10 → 9 tall. `Close` adds no extremum.
        assert_eq!(path.bounds(), Some(r(1, 2, 9, 9)));
    }

    #[test]
    fn a_single_point_path_is_one_pixel_not_zero_area() {
        let path = PathBuilder::new().move_to(Point::new(3, 7)).build();
        assert_eq!(path.bounds(), Some(r(3, 7, 1, 1)));
    }

    #[test]
    fn an_arc_contributes_its_whole_circle() {
        // Conservative on purpose (see `Path::bounds`): a 90° sweep still bounds
        // the full circle, because under-approximating would let a culler drop a
        // visible op.
        let path = PathBuilder::new()
            .arc_to(
                Point::new(20, 20),
                5,
                Angle::zero(),
                Angle::from_degrees(90.0),
            )
            .build();
        assert_eq!(path.bounds(), Some(r(15, 15, 11, 11)));
    }

    #[test]
    fn a_path_that_reaches_no_point_has_no_bounds() {
        assert_eq!(Path::default().bounds(), None);
        assert_eq!(PathBuilder::new().close().build().bounds(), None);
    }
}

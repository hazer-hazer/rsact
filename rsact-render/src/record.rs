//! A [`Renderer`] that records the draw operations it is asked to perform.
//!
//! Lets a test assert *what* was drawn and *where*, which a finished image
//! cannot show — it looks identical whether one rect or the whole screen was
//! repainted.
//!
//! The log is **geometry-only and color-agnostic**. Use a pixel snapshot to
//! check colors.

use crate::{
    color::Color,
    geometry::{Angle, CornerRadii, Point, Rect, Size},
    image::DrawImage,
    path::Path,
    renderer::{RenderResult, Renderer},
    style::DrawStyle,
};
use alloc::{rc::Rc, string::String, vec::Vec};
use core::{cell::RefCell, fmt, marker::PhantomData};

/// One recorded draw operation — geometry only (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawOp {
    /// A clipped region was entered; the ops that follow it (until its scope
    /// ends) drew inside `area`.
    Clip(Rect),
    FillSolid(Rect),
    Pixel(Point),
    Line {
        from: Point,
        to: Point,
    },
    Rect(Rect),
    RoundedRect(Rect),
    Circle {
        top_left: Point,
        diameter: u32,
    },
    Arc {
        top_left: Point,
        diameter: u32,
    },
    Ellipse(Rect),
    Sector {
        top_left: Point,
        diameter: u32,
    },
    /// The points are not kept, only their bounding box, which is what
    /// [`Self::bounds`] needs.
    Polygon {
        points: usize,
        bounds: Rect,
    },
    /// Ditto: [`Path::bounds`] at record time keeps the log `Copy`.
    Path {
        bounds: Rect,
    },
    Image {
        bounds: Rect,
    },
}

impl DrawOp {
    /// The conservative pixel bound of what this op draws, or `None` for an op
    /// that draws nothing.
    ///
    /// **The culling contract**, one-way: a culler may skip a region this bound
    /// misses, never one it hits.
    ///
    /// Two imprecisions bound what a check over it can prove. `None` means
    /// bookkeeping rather than "everywhere" — only [`Self::Clip`] returns it, so
    /// a *lost* clip is invisible here. And stroke width is not recorded, so a
    /// stroked primitive paints up to `stroke_width / 2` outside this.
    pub fn bounds(&self) -> Option<Rect> {
        // A `diameter`-wide primitive anchored at its top-left corner.
        let square = |top_left: Point, diameter: u32| {
            Rect::new(top_left, Size::new_equal(diameter))
        };

        match *self {
            // Bookkeeping, not drawing.
            DrawOp::Clip(_) => None,
            DrawOp::FillSolid(rect)
            | DrawOp::Rect(rect)
            | DrawOp::RoundedRect(rect)
            | DrawOp::Ellipse(rect)
            | DrawOp::Polygon { bounds: rect, .. }
            | DrawOp::Path { bounds: rect }
            | DrawOp::Image { bounds: rect } => Some(rect),
            DrawOp::Pixel(point) => Some(Rect::new(point, Size::new_equal(1))),
            DrawOp::Line { from, to } => Some(Rect::new(
                Point::new(from.x.min(to.x), from.y.min(to.y)),
                // Inclusive endpoints against an exclusive rect edge, so a
                // horizontal line is 1 pixel tall rather than zero-area.
                Size::new(
                    (from.x.max(to.x) - from.x.min(to.x) + 1) as u32,
                    (from.y.max(to.y) - from.y.min(to.y) + 1) as u32,
                ),
            )),
            DrawOp::Circle { top_left, diameter }
            | DrawOp::Arc { top_left, diameter }
            | DrawOp::Sector { top_left, diameter } => {
                Some(square(top_left, diameter))
            },
        }
    }
}

impl fmt::Display for DrawOp {
    /// One op per line, for golden comparison: a `Rect` renders as `x,y WxH`,
    /// a `Point` as `x,y`. Formatted by hand so the golden format cannot drift
    /// with the geometry types' own `Display`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Inline closures: reusing these across arms would need a wrapper.
        let rect = |f: &mut fmt::Formatter<'_>, r: Rect| {
            write!(
                f,
                "{},{} {}x{}",
                r.top_left.x, r.top_left.y, r.size.width, r.size.height
            )
        };
        let point =
            |f: &mut fmt::Formatter<'_>, p: Point| write!(f, "{},{}", p.x, p.y);

        match *self {
            DrawOp::Clip(r) => {
                write!(f, "Clip ")?;
                rect(f, r)
            },
            DrawOp::FillSolid(r) => {
                write!(f, "FillSolid ")?;
                rect(f, r)
            },
            DrawOp::Pixel(p) => {
                write!(f, "Pixel ")?;
                point(f, p)
            },
            DrawOp::Line { from, to } => {
                write!(f, "Line ")?;
                point(f, from)?;
                write!(f, " -> ")?;
                point(f, to)
            },
            DrawOp::Rect(r) => {
                write!(f, "Rect ")?;
                rect(f, r)
            },
            DrawOp::RoundedRect(r) => {
                write!(f, "RoundedRect ")?;
                rect(f, r)
            },
            DrawOp::Circle { top_left, diameter } => {
                write!(f, "Circle ")?;
                point(f, top_left)?;
                write!(f, " d={diameter}")
            },
            DrawOp::Arc { top_left, diameter } => {
                write!(f, "Arc ")?;
                point(f, top_left)?;
                write!(f, " d={diameter}")
            },
            DrawOp::Ellipse(r) => {
                write!(f, "Ellipse ")?;
                rect(f, r)
            },
            DrawOp::Sector { top_left, diameter } => {
                write!(f, "Sector ")?;
                point(f, top_left)?;
                write!(f, " d={diameter}")
            },
            // These three carry no position of their own, so without the
            // bound a golden cannot tell a placed icon from a displaced one.
            DrawOp::Polygon { points, bounds } => {
                write!(f, "Polygon n={points} ")?;
                rect(f, bounds)
            },
            DrawOp::Path { bounds } => {
                write!(f, "Path ")?;
                rect(f, bounds)
            },
            DrawOp::Image { bounds } => {
                write!(f, "Image ")?;
                rect(f, bounds)
            },
        }
    }
}

/// Serialise a draw-op log to the newline-terminated text used as golden
/// content — see [`DrawOp`]'s `Display`.
pub fn format_ops(ops: &[DrawOp]) -> String {
    use fmt::Write as _;
    let mut out = String::new();
    for op in ops {
        // Writing into a String is infallible; the Result is discarded.
        let _ = writeln!(out, "{op}");
    }
    out
}

/// A [`Renderer`] that logs its draw operations. Clones share one log, so a
/// copy handed to the render pass records into the buffer the test reads.
/// `P` is the [frame policy](crate::region::FramePolicy) this recorder reports,
/// defaulting to [`Unbounded`](crate::region::Unbounded).
///
/// A recorder has no surface, so the parameter lets it stand in for any
/// target's region bound while still recording every op.
pub struct RecordingRenderer<C, P = crate::region::Unbounded> {
    size: Size,
    ops: Rc<RefCell<Vec<DrawOp>>>,
    /// A real clip stack, so [`Renderer::clip_bounds`] reports something a
    /// culler can use. Clips are recorded, not applied. Shared with clones.
    clips: Rc<RefCell<Vec<Rect>>>,
    _color: PhantomData<C>,
    _policy: PhantomData<P>,
}

impl<C, P> Clone for RecordingRenderer<C, P> {
    fn clone(&self) -> Self {
        Self {
            size: self.size,
            ops: Rc::clone(&self.ops),
            clips: Rc::clone(&self.clips),
            _color: PhantomData,
            _policy: PhantomData,
        }
    }
}

impl<C, P> RecordingRenderer<C, P> {
    pub fn new(size: Size) -> Self {
        Self {
            size,
            ops: Rc::new(RefCell::new(Vec::new())),
            clips: Rc::new(RefCell::new(vec![Rect::new(Point::zero(), size)])),
            _color: PhantomData,
            _policy: PhantomData,
        }
    }

    /// A snapshot of the recorded ops in draw order.
    pub fn ops(&self) -> Vec<DrawOp> {
        self.ops.borrow().clone()
    }

    /// Number of ops recorded so far.
    pub fn len(&self) -> usize {
        self.ops.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.ops.borrow().is_empty()
    }

    /// Drop the log, to record just the next frame.
    pub fn clear(&self) {
        self.ops.borrow_mut().clear();
    }

    fn push(&self, op: DrawOp) {
        self.ops.borrow_mut().push(op);
    }

    /// The effective clip: the top of the stack, whose root is the recorder's
    /// own extent so a full-frame capture reports what a real backend would.
    fn current_clip(&self) -> Rect {
        // Non-empty by construction; `pop_clip` never empties it.
        self.clips
            .borrow()
            .last()
            .copied()
            .unwrap_or(Rect::new(Point::zero(), self.size))
    }
}

/// The bounding box of a point set. `None` for empty input, which draws
/// nothing.
fn points_bounds(points: &[Point]) -> Rect {
    let Some(first) = points.first() else {
        return Rect::zero();
    };
    let (mut min, mut max) = (*first, *first);
    for point in &points[1..] {
        min = Point::new(min.x.min(point.x), min.y.min(point.y));
        max = Point::new(max.x.max(point.x), max.y.max(point.y));
    }
    // Inclusive corners, exclusive rect edge (see `Path::bounds`).
    Rect::new(
        min,
        Size::new((max.x - min.x + 1) as u32, (max.y - min.y + 1) as u32),
    )
}

impl<C: Color, P: crate::region::FramePolicy> Renderer
    for RecordingRenderer<C, P>
{
    type Color = C;

    /// Whatever the harness asked for; the recorder never chunks anything.
    type Policy = P;

    fn size(&self) -> Size {
        self.size
    }

    fn push_clip(&mut self, area: Rect) {
        // The log records what was ASKED for; the stack stores the narrowed
        // rect, which is what `clip_bounds` must report.
        self.push(DrawOp::Clip(area));
        let nested = area.intersection(&self.current_clip());
        self.clips.borrow_mut().push(nested);
    }

    // Records NOTHING: the log is a linear trace in which a `Clip` applies to
    // the ops that follow it, so an "unclip" marker would add no information.
    fn pop_clip(&mut self) {
        // Never pops the root: an unbalanced pop must degrade.
        let mut clips = self.clips.borrow_mut();
        if clips.len() > 1 {
            clips.pop();
        }
    }

    fn clip_bounds(&self) -> Option<Rect> {
        Some(self.current_clip())
    }

    fn fill_solid(&mut self, rect: Rect, _color: Self::Color) -> RenderResult {
        self.push(DrawOp::FillSolid(rect));
        Ok(())
    }

    fn pixel(&mut self, point: Point, _color: Self::Color) -> RenderResult {
        self.push(DrawOp::Pixel(point));
        Ok(())
    }

    fn line(
        &mut self,
        from: Point,
        to: Point,
        _style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        self.push(DrawOp::Line { from, to });
        Ok(())
    }

    fn rect(
        &mut self,
        rect: Rect,
        _style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        self.push(DrawOp::Rect(rect));
        Ok(())
    }

    fn rounded_rect(
        &mut self,
        rect: Rect,
        _corners: CornerRadii,
        _style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        self.push(DrawOp::RoundedRect(rect));
        Ok(())
    }

    fn circle(
        &mut self,
        top_left: Point,
        diameter: u32,
        _style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        self.push(DrawOp::Circle { top_left, diameter });
        Ok(())
    }

    fn arc(
        &mut self,
        top_left: Point,
        diameter: u32,
        _start: Angle,
        _sweep: Angle,
        _style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        self.push(DrawOp::Arc { top_left, diameter });
        Ok(())
    }

    fn ellipse(
        &mut self,
        bounding_box: Rect,
        _style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        self.push(DrawOp::Ellipse(bounding_box));
        Ok(())
    }

    fn sector(
        &mut self,
        top_left: Point,
        diameter: u32,
        _start: Angle,
        _sweep: Angle,
        _style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        self.push(DrawOp::Sector { top_left, diameter });
        Ok(())
    }

    fn polygon(
        &mut self,
        points: &[Point],
        _style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        self.push(DrawOp::Polygon {
            points: points.len(),
            bounds: points_bounds(points),
        });
        Ok(())
    }

    fn path(
        &mut self,
        path: &Path,
        _style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        // Draws nothing, and `Rect::zero` intersects no region.
        self.push(DrawOp::Path {
            bounds: path.bounds().unwrap_or(Rect::zero()),
        });
        Ok(())
    }

    fn image<'a>(&mut self, image: DrawImage<'a, Self::Color>) -> RenderResult {
        self.push(DrawOp::Image { bounds: image.bounding_box() });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::NullColor;

    fn r(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect::new(Point::new(x, y), Size::new(w, h))
    }

    /// The recorder records clips rather than applying them, but must still
    /// *report* the effective one — a harness that saw `None` would cull nothing
    /// and measure the un-culled cost as though it were culled.
    #[test]
    fn clip_bounds_reports_the_effective_clip() {
        let mut rec = RecordingRenderer::<NullColor>::new(Size::new_equal(64));
        let surface = r(0, 0, 64, 64);
        assert_eq!(
            rec.clip_bounds(),
            Some(surface),
            "the root reports the SURFACE rect, not `None` — that is what makes \
             culling pay on a full-frame render and not only under tiles"
        );

        rec.push_clip(r(0, 0, 20, 20));
        assert_eq!(rec.clip_bounds(), Some(r(0, 0, 20, 20)));
        // Reaches beyond its parent ⇒ the effective clip is the intersection.
        rec.push_clip(r(10, 10, 20, 20));
        assert_eq!(rec.clip_bounds(), Some(r(10, 10, 10, 10)));

        rec.pop_clip();
        assert_eq!(rec.clip_bounds(), Some(r(0, 0, 20, 20)));
        rec.pop_clip();
        assert_eq!(rec.clip_bounds(), Some(surface));
        // Unbalanced pop degrades rather than panicking.
        rec.pop_clip();
        assert_eq!(rec.clip_bounds(), Some(surface));

        // None of it touches the op log, so the goldens stay valid.
        assert_eq!(
            rec.ops(),
            [DrawOp::Clip(r(0, 0, 20, 20)), DrawOp::Clip(r(10, 10, 20, 20))],
            "the log records the REQUESTED clips, not the narrowed ones"
        );
    }

    /// A renderer that reports no clip must disable culling, not enable it with
    /// a zero rect — `NullRenderer::size()` is zero, which would read as "clips
    /// everything away".
    #[test]
    fn a_non_reporting_renderer_disables_culling() {
        use crate::renderer::NullRenderer;
        assert_eq!(NullRenderer::<NullColor>::default().clip_bounds(), None);
    }

    #[test]
    fn records_ops_in_draw_order() {
        let mut rec = RecordingRenderer::<NullColor>::new(Size::new_equal(64));
        assert!(rec.is_empty());

        rec.fill_solid(r(0, 0, 10, 10), NullColor).unwrap();
        rec.rect(r(2, 2, 4, 4), &DrawStyle::default()).unwrap();

        assert_eq!(
            rec.ops(),
            [DrawOp::FillSolid(r(0, 0, 10, 10)), DrawOp::Rect(r(2, 2, 4, 4))]
        );
    }

    #[test]
    fn clipped_records_the_region_then_the_nested_ops() {
        let mut rec = RecordingRenderer::<NullColor>::new(Size::new_equal(64));
        rec.push_clip(r(1, 1, 8, 8));
        rec.fill_solid(r(2, 2, 3, 3), NullColor).unwrap();
        rec.pop_clip();

        // The clip is logged before the ops it applies to, which is what lets
        // a test assert "drawing was confined to the damage rect".
        assert_eq!(
            rec.ops(),
            [DrawOp::Clip(r(1, 1, 8, 8)), DrawOp::FillSolid(r(2, 2, 3, 3))]
        );
    }

    #[test]
    fn format_ops_is_stable_one_line_per_op() {
        let ops = [
            DrawOp::Clip(r(1, 2, 8, 8)),
            DrawOp::FillSolid(r(0, 0, 10, 20)),
            DrawOp::Pixel(Point::new(3, 4)),
            DrawOp::Line { from: Point::new(0, 0), to: Point::new(5, 6) },
            DrawOp::Rect(r(2, 2, 4, 4)),
            DrawOp::RoundedRect(r(2, 2, 4, 4)),
            DrawOp::Circle { top_left: Point::new(5, 5), diameter: 10 },
            DrawOp::Arc { top_left: Point::new(5, 5), diameter: 10 },
            DrawOp::Ellipse(r(1, 1, 6, 3)),
            DrawOp::Sector { top_left: Point::new(5, 5), diameter: 10 },
            DrawOp::Polygon { points: 3, bounds: r(1, 1, 6, 6) },
            DrawOp::Path { bounds: r(12, 13, 8, 8) },
            DrawOp::Image { bounds: r(0, 0, 16, 16) },
        ];

        assert_eq!(
            format_ops(&ops),
            "\
Clip 1,2 8x8
FillSolid 0,0 10x20
Pixel 3,4
Line 0,0 -> 5,6
Rect 2,2 4x4
RoundedRect 2,2 4x4
Circle 5,5 d=10
Arc 5,5 d=10
Ellipse 1,1 6x3
Sector 5,5 d=10
Polygon n=3 1,1 6x6
Path 12,13 8x8
Image 0,0 16x16
"
        );
    }

    /// The three bounds easy to get wrong: an inclusive-endpoint line must not
    /// collapse to zero area, a `Pixel` covers exactly one, and a `Clip` none.
    #[test]
    fn bounds_are_the_conservative_pixel_extent() {
        assert_eq!(
            DrawOp::FillSolid(r(3, 4, 10, 20)).bounds(),
            Some(r(3, 4, 10, 20))
        );
        assert_eq!(
            DrawOp::Pixel(Point::new(7, 9)).bounds(),
            Some(r(7, 9, 1, 1))
        );
        // One pixel tall: a zero-area bound would oblige nobody to draw it.
        assert_eq!(
            DrawOp::Line { from: Point::new(2, 5), to: Point::new(8, 5) }
                .bounds(),
            Some(r(2, 5, 7, 1))
        );
        // Endpoint order must not matter.
        assert_eq!(
            DrawOp::Line { from: Point::new(8, 9), to: Point::new(2, 5) }
                .bounds(),
            Some(r(2, 5, 7, 5))
        );
        assert_eq!(
            DrawOp::Circle { top_left: Point::new(5, 5), diameter: 10 }
                .bounds(),
            Some(r(5, 5, 10, 10))
        );
        // Bookkeeping: a clip draws nothing.
        assert_eq!(DrawOp::Clip(r(0, 0, 64, 64)).bounds(), None);
    }

    /// A polygon's extent is captured at record time, including the
    /// inclusive-corner `+1` a collinear polygon needs to be non-empty.
    #[test]
    fn polygon_records_its_extent_not_its_points() {
        let mut rec = RecordingRenderer::<NullColor>::new(Size::new_equal(64));
        rec.polygon(
            &[Point::new(4, 10), Point::new(9, 2), Point::new(1, 6)],
            &DrawStyle::default(),
        )
        .unwrap();

        assert_eq!(
            rec.ops(),
            [DrawOp::Polygon { points: 3, bounds: r(1, 2, 9, 9) }]
        );
    }

    #[test]
    fn empty_polygon_has_a_zero_bound() {
        let mut rec = RecordingRenderer::<NullColor>::new(Size::new_equal(64));
        rec.polygon(&[], &DrawStyle::default()).unwrap();
        assert_eq!(
            rec.ops(),
            [DrawOp::Polygon { points: 0, bounds: Rect::zero() }]
        );
    }

    #[test]
    fn clones_share_one_log() {
        let rec = RecordingRenderer::<NullColor>::new(Size::new_equal(64));
        let mut handed_to_pass = rec.clone();
        handed_to_pass.pixel(Point::new(3, 4), NullColor).unwrap();
        // The test's original handle sees what the pass recorded.
        assert_eq!(rec.ops(), [DrawOp::Pixel(Point::new(3, 4))]);
    }
}

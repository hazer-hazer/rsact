//! A [`Renderer`] that records the draw operations it is asked to perform, for
//! golden ("blessed reference") render tests (WS6.9).
//!
//! Unlike [`NullRenderer`](crate::renderer::NullRenderer) — a pure no-op — this
//! keeps a shared, ordered log of every primitive and clip region, so a test can
//! assert *what* was drawn and *where*. That is exactly the signal WS6's
//! damage-driven rendering needs: "only this rect was touched" is a statement
//! about the draw log, not about the final image (the image is identical whether
//! you repaint one rect or the whole screen).
//!
//! The log is **geometry-focused and colour-agnostic** on purpose: it records
//! positions, sizes, primitive kinds and clip regions — the WS6 damage signal —
//! not exact colours, so it stays deterministic and generic over any [`Color`].
//! Visual (colour / anti-aliasing) correctness is the tiny-skia PNG snapshot's
//! job, not this one's.

use crate::{
    color::Color,
    geometry::{Angle, CornerRadii, Point, Rect, Size},
    image::DrawImage,
    path::Path,
    renderer::{RenderResult, Renderer, ViewportKind},
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
    /// `bounds` is the polygon's own bounding box: the individual points are not
    /// kept (the log is a *count* of primitives, not a copy of their input), but
    /// the extent is, because WS6.4a's [`Self::bounds`] contract needs it.
    Polygon {
        points: usize,
        bounds: Rect,
    },
    /// Ditto — [`Path::bounds`] at record time, so the log stays `Copy` and
    /// allocation-free while remaining geometrically checkable.
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
    /// **This is the culling contract** (WS6.4a). Read it as an obligation in one
    /// direction: *if* this bound intersects a region, a renderer replaying the
    /// frame region-by-region **must** emit the op for that region. A culler is
    /// free to skip a region the bound misses; it may never skip one the bound
    /// hits. [`tile_invariance`] is that sentence turned into an assertion, so
    /// this method and any future geometric cull must stay the *same* predicate —
    /// a cull tighter than this bound would pass review and fail on screen.
    ///
    /// Two deliberate imprecisions, both recorded because they set the limits of
    /// what the check can prove:
    ///
    /// - **`None` means bookkeeping, not "everywhere".** Only [`Self::Clip`]
    ///   returns it: a clip paints nothing, so it carries no obligation, and
    ///   WS6.4a's arithmetic skips such ops entirely rather than treating them as
    ///   present-in-every-tile. A *lost clip* is therefore invisible to the
    ///   invariance check — that failure mode is over-painting, which is the
    ///   pixel goldens' business (WS6.9's deferred PNG half).
    /// - **Stroke width is not recorded** (the log is style-agnostic on purpose),
    ///   so a stroked primitive paints up to `stroke_width / 2` outside its
    ///   geometry. The bound therefore *under*-approximates by that margin, which
    ///   makes the invariance check slightly weaker (it can miss a genuinely lost
    ///   op in a boundary sliver) but never wrong in the other direction. Fixing
    ///   it would mean recording style, which is exactly what keeps this log
    ///   deterministic and colour-agnostic.
    ///
    /// [`tile_invariance`]: crate::schedule::tile_invariance
    /// [`Path::bounds`]: crate::path::Path::bounds
    pub fn bounds(&self) -> Option<Rect> {
        // A `diameter`-wide primitive anchored at its top-left corner.
        let square = |top_left: Point, diameter: u32| {
            Rect::new(top_left, Size::new_equal(diameter))
        };

        match *self {
            // Bookkeeping, not drawing — see the note above.
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
                // Inclusive endpoints, exclusive rect edge — hence `+ 1`, so a
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
    /// A stable, one-op-per-line textual form for golden comparison. Formatted
    /// by hand (not via the geometry types' own `Display`) so the golden format
    /// is under this module's control and can't drift if `Rect`/`Point` change
    /// how they print. A `Rect` renders as `x,y WxH`, a `Point` as `x,y`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Local formatters — a `Rect`/`Point` argument would need its own
        // wrapper type to reuse across arms, so inline closures keep it simple.
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
            // WS6.4a prints the bound for the three ops whose own line carries
            // no position at all. `Path` in particular used to log as the bare
            // word "Path", so the checkbox goldens could not tell a correctly
            // placed check-icon from a displaced one. The two WS6.9 goldens are
            // re-blessed in the same commit (as `pop_clip`'s note requires).
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

/// Serialise a draw-op log to the stable, newline-terminated, one-op-per-line
/// text used as the golden content (see [`DrawOp`]'s `Display`). This is the
/// draw-call side of the WS6.9 harness: the exact primitives + positions a
/// render pass emitted, comparable across runs and colour-agnostic.
pub fn format_ops(ops: &[DrawOp]) -> String {
    use fmt::Write as _;
    let mut out = String::new();
    for op in ops {
        // Writing into a String is infallible; the Result is discarded.
        let _ = writeln!(out, "{op}");
    }
    out
}

/// A [`Renderer`] that logs its draw operations for golden tests. Cheap to clone
/// — clones share one log (`Rc<RefCell<..>>`), so a copy handed to the render
/// pass records into the same buffer the test reads.
/// `P` is the [frame policy](crate::region::FramePolicy) this recorder reports,
/// defaulting to [`Unbounded`](crate::region::Unbounded).
///
/// A recorder has no surface, so no policy is forced on it — but the frame
/// planner reads the policy from the *renderer type*, and a harness measuring
/// how a schedule behaves under `Tiles<240, 24>` needs a renderer that asks for
/// `Tiles<240, 24>`. Making it a parameter is what lets one recorder stand in
/// for any target's region bound while still recording every op, unclipped and
/// unchunked by any real storage.
pub struct RecordingRenderer<C, P = crate::region::Unbounded> {
    size: Size,
    ops: Rc<RefCell<Vec<DrawOp>>>,
    /// WS6.4b: a real clip stack, so [`Renderer::clip_bounds`] can report the
    /// effective clip. The recorder does not *apply* clips (it records what the
    /// drawing code asked for, which is the measurement), but culling reads the
    /// clip, so the harness would see `None` and cull nothing without this.
    /// Shared with clones, like the log.
    clips: Rc<RefCell<Vec<ViewportKind>>>,
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
            clips: Rc::new(RefCell::new(vec![ViewportKind::root()])),
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

    /// Drop the log (e.g. between frames, to record just the next one).
    pub fn clear(&self) {
        self.ops.borrow_mut().clear();
    }

    fn push(&self, op: DrawOp) {
        self.ops.borrow_mut().push(op);
    }

    fn current_viewport(&self) -> ViewportKind {
        // The stack is created non-empty and `pop_clip` never empties it.
        self.clips
            .borrow()
            .last()
            .copied()
            .unwrap_or_else(ViewportKind::root)
    }
}

/// The bounding box of a point set — the recorded extent of a polygon, whose
/// individual points the log does not keep. Empty input has no extent, and a
/// zero-sized rect intersects nothing, which is the right answer for a primitive
/// that draws nothing.
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

    /// Whatever policy the harness asked for — see the type's own docs. The
    /// recorder never chunks anything itself, so what a test observes is exactly
    /// the schedule the planner chose under that policy.
    type Policy = P;

    fn size(&self) -> Size {
        self.size
    }

    fn push_clip(&mut self, area: Rect) {
        self.push(DrawOp::Clip(area));
        // Narrowed by the active clip, so the top IS the effective clip
        // (WS6.4b — see `ViewportKind::nested_in`).
        let nested =
            ViewportKind::Clipped(area).nested_in(self.current_viewport());
        self.clips.borrow_mut().push(nested);
    }

    // Deliberately records NOTHING (WS6.4.0(ii-1)). The op log is a linear
    // trace in which a `Clip` applies to the ops that follow it, so emitting an
    // "unclip" marker would change every WS6.9 golden for no information gain —
    // the previous closure form recorded no end marker either. If 6.4a's
    // tile-invariance check ever needs clip *scope* rather than clip *order*,
    // add the marker there and bless the goldens in the same commit.
    fn pop_clip(&mut self) {
        // Never pops the root, mirroring `EGRenderer`: an unbalanced pop must
        // degrade, not leave the renderer with no viewport at all.
        let mut clips = self.clips.borrow_mut();
        if clips.len() > 1 {
            clips.pop();
        }
    }

    fn clip_bounds(&self) -> Option<Rect> {
        // Fullscreen ⇒ the recorder's own extent, so a harness measuring a
        // full-frame capture sees the same cull rect a real backend would.
        Some(
            self.current_viewport()
                .clip_bounds()
                .unwrap_or(Rect::new(Point::zero(), self.size)),
        )
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
        // A path that reaches no point draws nothing; `Rect::zero` is exactly
        // that in bound form (it intersects no region).
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

    /// WS6.4b: the recorder does not *apply* clips — it records what the drawing
    /// code asked for, which is the measurement — but it must still *report* the
    /// effective clip, because that is what culling reads. Without a real stack
    /// here the tile harness would see `None`, cull nothing, and silently measure
    /// the un-culled cost as though it were the culled one.
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
        // Unbalanced pop degrades rather than panicking (as `EGRenderer` does).
        rec.pop_clip();
        assert_eq!(rec.clip_bounds(), Some(surface));

        // None of it touches the op log — the WS6.9 goldens stay valid.
        assert_eq!(
            rec.ops(),
            [DrawOp::Clip(r(0, 0, 20, 20)), DrawOp::Clip(r(10, 10, 20, 20))],
            "the log records the REQUESTED clips, not the narrowed ones"
        );
    }

    /// A renderer that does not report a clip must disable culling, not enable it
    /// with a zero rect: `NullRenderer::size()` is `Size::zero()`, so a default of
    /// `Rect::new(zero, size())` would read as "clips everything away" and cull
    /// every widget on every headless page.
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

        // The clip region is logged before the ops that drew inside it — this is
        // what lets WS6 assert "drawing was confined to the damage rect".
        //
        // WS6.4.0(ii-1): `pop_clip` deliberately records nothing, so this log —
        // and every WS6.9 golden — is byte-identical to the closure-based form
        // it replaces. The trace is linear: a `Clip` applies to the ops that
        // follow it.
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

    /// WS6.4a: [`DrawOp::bounds`] is the culling contract, so the arithmetic that
    /// derives a bound from a primitive's own anchor is pinned here. The three
    /// cases that are easy to get wrong: an inclusive-endpoint line must not
    /// collapse to zero area, a `Pixel` covers exactly one, and a `Clip` carries
    /// no obligation at all.
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
        // A horizontal line is one pixel TALL, not zero-area — a zero-area bound
        // would intersect no tile and so oblige nobody to draw it.
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

    /// A polygon's points are not kept in the log, so its extent must be captured
    /// at record time — including the inclusive-corner `+1`, without which a
    /// flat (collinear) polygon would record a zero-area bound.
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

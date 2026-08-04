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
    output::{FinishRender, RenderTarget, pixel::Pixel},
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
    Polygon {
        points: usize,
    },
    Path,
    Image,
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
            DrawOp::Polygon { points } => write!(f, "Polygon n={points}"),
            DrawOp::Path => write!(f, "Path"),
            DrawOp::Image => write!(f, "Image"),
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
pub struct RecordingRenderer<C> {
    size: Size,
    ops: Rc<RefCell<Vec<DrawOp>>>,
    _color: PhantomData<C>,
}

impl<C> Clone for RecordingRenderer<C> {
    fn clone(&self) -> Self {
        Self { size: self.size, ops: Rc::clone(&self.ops), _color: PhantomData }
    }
}

impl<C> RecordingRenderer<C> {
    pub fn new(size: Size) -> Self {
        Self {
            size,
            ops: Rc::new(RefCell::new(Vec::new())),
            _color: PhantomData,
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
}

impl<C: Color> RenderTarget for RecordingRenderer<C> {
    type Color = C;

    fn draw(&mut self, _pixels: impl Iterator<Item = Pixel<Self::Color>>) {}
}

// The finish target's colour is independent of the recorder's own colour.
impl<C, D> FinishRender<D> for RecordingRenderer<C> {
    fn finish_frame(&mut self, _target: &mut impl RenderTarget<Color = D>) {}
}

impl<C: Color> Renderer for RecordingRenderer<C> {
    type Color = C;

    fn size(&self) -> Size {
        self.size
    }

    fn push_clip(&mut self, area: Rect) {
        self.push(DrawOp::Clip(area));
    }

    // Deliberately records NOTHING (WS6.4.0(ii-1)). The op log is a linear
    // trace in which a `Clip` applies to the ops that follow it, so emitting an
    // "unclip" marker would change every WS6.9 golden for no information gain —
    // the previous closure form recorded no end marker either. If 6.4a's
    // tile-invariance check ever needs clip *scope* rather than clip *order*,
    // add the marker there and bless the goldens in the same commit.
    fn pop_clip(&mut self) {}

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
        self.push(DrawOp::Polygon { points: points.len() });
        Ok(())
    }

    fn path(
        &mut self,
        _path: &Path,
        _style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        self.push(DrawOp::Path);
        Ok(())
    }

    fn image<'a>(
        &mut self,
        _image: DrawImage<'a, Self::Color>,
    ) -> RenderResult {
        self.push(DrawOp::Image);
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
            DrawOp::Polygon { points: 3 },
            DrawOp::Path,
            DrawOp::Image,
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
Polygon n=3
Path
Image
"
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

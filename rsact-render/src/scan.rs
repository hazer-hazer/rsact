//! Shared scan conversion — the bodies every [`Rasterizer`](crate::raster::Rasterizer) default delegates
//! to.
//!
//! Free functions, so they monomorphize over the blitter alone rather than once
//! per rasterizer.
//!
//! Only [`rect`], [`line`], [`polygon`]'s scan and [`image`]'s row decode are
//! real scan conversion; [`arc`], [`sector`], [`ellipse`] and [`rounded_rect`]
//! flatten to a polyline and reuse [`polygon`].
//!
//! Everything here is aliased and integer-exact — the floor a rasterizer stands
//! on before it overrides anything.
//!
//! [`Rasterizer`]: crate::raster::Rasterizer

#[allow(unused)]
use crate::FloatExt as _;
use crate::{
    blitter::{Blitter, Span},
    color::Color,
    geometry::{Angle, CornerRadii, Point, PointExt as _, Rect, Size},
    image::DrawImage,
    path::{Path, PathSegment},
    raster::RasterCtx,
    style::{DrawStyle, StrokeAlignment},
};
use alloc::{vec, vec::Vec};

/// Pixels of arc per flattened segment. Smaller is smoother and slower; 2 px
/// puts a 100 px-radius quarter-circle at ~79 segments.
const FLATTEN_STEP_PX: f32 = 2.0;

// ───────────────────────────────────────────────────────── style helpers

/// The stroke width that will actually be drawn. A width without a color draws
/// nothing **and must not shrink the fill**.
fn effective_stroke<C: Color>(style: &DrawStyle<C>) -> Option<(C, u32)> {
    match (style.stroke, style.stroke_width) {
        (Some(color), width @ 1..) => Some((color, width)),
        _ => None,
    }
}

fn grow(rect: Rect, by: u32) -> Rect {
    if by == 0 {
        return rect;
    }
    Rect::new(
        Point::new(
            rect.top_left.x.saturating_sub(by as i32),
            rect.top_left.y.saturating_sub(by as i32),
        ),
        Size::new(
            rect.size.width.saturating_add(by * 2),
            rect.size.height.saturating_add(by * 2),
        ),
    )
}

fn shrink(rect: Rect, by: u32) -> Rect {
    let both = by.saturating_mul(2);
    if by == 0 {
        return rect;
    }
    if rect.size.width <= both || rect.size.height <= both {
        return Rect::zero();
    }
    Rect::new(
        Point::new(rect.top_left.x + by as i32, rect.top_left.y + by as i32),
        Size::new(rect.size.width - both, rect.size.height - both),
    )
}

/// The outer and inner edges of a `width`-thick stroke around `rect`. A
/// `Center` stroke of odd width puts the extra pixel **inside**.
fn stroke_edges(
    rect: Rect,
    width: u32,
    alignment: StrokeAlignment,
) -> (Rect, Rect) {
    match alignment {
        StrokeAlignment::Inside => (rect, shrink(rect, width)),
        StrokeAlignment::Center => {
            (grow(rect, width / 2), shrink(rect, width - width / 2))
        },
        StrokeAlignment::Outside => (grow(rect, width), rect),
    }
}

// ───────────────────────────────────────────────────────── the primitives

/// Style-free rect fill — clears, backgrounds, region priming.
pub fn fill<T: Blitter>(
    cx: &mut RasterCtx<'_, T>,
    rect: Rect,
    color: T::Color,
) {
    cx.rect(rect, color)
}

/// A styled rectangle: the fill, then the stroke as four bands. Bands rather
/// than lines — a 1 px border on a 240-wide frame is 2 spans plus 2·h, not
/// 2·240 pixels.
pub fn rect<T: Blitter>(
    cx: &mut RasterCtx<'_, T>,
    rect: Rect,
    style: &DrawStyle<T::Color>,
) {
    let stroke = effective_stroke(style);
    let (outer, inner) = match stroke {
        Some((_, width)) => stroke_edges(rect, width, style.stroke_alignment),
        None => (rect, rect),
    };

    if let Some(fill_color) = style.fill {
        cx.rect(inner, fill_color);
    }

    let Some((stroke_color, _)) = stroke else { return };

    if inner.is_zero_sized() {
        // The stroke swallowed the shape: one solid rect.
        cx.rect(outer, stroke_color);
        return;
    }

    let (ox, oy) = (outer.top_left.x, outer.top_left.y);
    let (ow, oh) = (outer.size.width, outer.size.height);
    let (ix, iy) = (inner.top_left.x, inner.top_left.y);
    let (iw, ih) = (inner.size.width, inner.size.height);

    // Left and right fill only the gap, so no pixel is written twice.
    cx.rect(
        Rect::new(Point::new(ox, oy), Size::new(ow, (iy - oy) as u32)),
        stroke_color,
    );
    cx.rect(
        Rect::new(
            Point::new(ox, iy + ih as i32),
            Size::new(ow, ((oy + oh as i32) - (iy + ih as i32)) as u32),
        ),
        stroke_color,
    );
    cx.rect(
        Rect::new(Point::new(ox, iy), Size::new((ix - ox) as u32, ih)),
        stroke_color,
    );
    cx.rect(
        Rect::new(
            Point::new(ix + iw as i32, iy),
            Size::new(((ox + ow as i32) - (ix + iw as i32)) as u32, ih),
        ),
        stroke_color,
    );
}

/// A straight line of `stroke_width`, centred on the segment. A line has no
/// interior, so `style.fill` is ignored.
///
/// Axis-aligned lines take a rect path — borders, separators and dividers are
/// most of the lines a UI draws, and each is one `fill_rect` rather than
/// `length × width` pixels.
pub fn line<T: Blitter>(
    cx: &mut RasterCtx<'_, T>,
    from: Point,
    to: Point,
    style: &DrawStyle<T::Color>,
) {
    let Some((color, width)) = effective_stroke(style) else { return };
    let half = (width / 2) as i32;

    if from.y == to.y {
        let x = from.x.min(to.x);
        let w = (from.x - to.x).unsigned_abs() + 1;
        cx.rect(
            Rect::new(Point::new(x, from.y - half), Size::new(w, width)),
            color,
        );
        return;
    }
    if from.x == to.x {
        let y = from.y.min(to.y);
        let h = (from.y - to.y).unsigned_abs() + 1;
        cx.rect(
            Rect::new(Point::new(from.x - half, y), Size::new(width, h)),
            color,
        );
        return;
    }

    // Bresenham, thickened perpendicular to the MAJOR AXIS rather than to the
    // line, which narrows a diagonal stroke by up to √2. Correcting that is
    // stroke-outline work.
    let dx = (to.x - from.x).abs();
    let dy = (to.y - from.y).abs();
    let sx = if from.x < to.x { 1 } else { -1 };
    let sy = if from.y < to.y { 1 } else { -1 };
    let steep = dy > dx;
    let mut err = dx - dy;
    let (mut x, mut y) = (from.x, from.y);

    loop {
        if steep {
            cx.span(Span::new(y, x - half, width), color);
        } else {
            for k in 0..width as i32 {
                cx.pixel(Point::new(x, y - half + k), color);
            }
        }

        if x == to.x && y == to.y {
            break;
        }
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }
}

/// A closed polygon: a winding-rule fill, then its edges.
///
/// The fill is `O(w·h·edges)`, testing every pixel of the bounding box against a
/// winding number; a scanline fill emitting spans is the natural rewrite. The
/// scan is bounded by `cx.clip()` rather than the polygon's own box.
pub fn polygon<T: Blitter>(
    cx: &mut RasterCtx<'_, T>,
    points: &[Point],
    style: &DrawStyle<T::Color>,
) {
    if let Some(fill_color) = style.fill {
        fill_polygon(cx, points, fill_color);
    }
    if effective_stroke(style).is_some() && points.len() >= 2 {
        for i in 0..points.len() {
            line(cx, points[i], points[(i + 1) % points.len()], style);
        }
    }
}

/// A path: fill each subpath, then stroke it. Flattening first is what keeps
/// `ArcTo` honest — the arc is generated about its stated centre and the cursor
/// is the last point emitted.
pub fn path<T: Blitter>(
    cx: &mut RasterCtx<'_, T>,
    path: &Path,
    style: &DrawStyle<T::Color>,
) {
    for sub in flatten(path) {
        if let Some(fill_color) = style.fill {
            fill_polygon(cx, &sub.points, fill_color);
        }
        if effective_stroke(style).is_some() {
            for pair in sub.points.windows(2) {
                line(cx, pair[0], pair[1], style);
            }
            if sub.closed && sub.points.len() > 2 {
                line(
                    cx,
                    sub.points[sub.points.len() - 1],
                    sub.points[0],
                    style,
                );
            }
        }
    }
}

/// An arc — a **curve**, so `style.fill` is ignored.
///
/// `top_left` and `diameter` describe the circle's bounding box, as everywhere
/// else in this trait. Angle zero is `+x` and a positive `sweep` runs toward
/// `+y`, i.e. clockwise on screen.
pub fn arc<T: Blitter>(
    cx: &mut RasterCtx<'_, T>,
    top_left: Point,
    diameter: u32,
    start: Angle,
    sweep: Angle,
    style: &DrawStyle<T::Color>,
) {
    if effective_stroke(style).is_none() {
        return;
    }
    let r = diameter as f32 / 2.0;
    let center = Point::new(
        top_left.x + (diameter / 2) as i32,
        top_left.y + (diameter / 2) as i32,
    );
    let points = arc_points(center, r, r, start, sweep);
    for pair in points.windows(2) {
        line(cx, pair[0], pair[1], style);
    }
}

/// A sector — the pie slice, so unlike [`arc`] it has an interior.
pub fn sector<T: Blitter>(
    cx: &mut RasterCtx<'_, T>,
    top_left: Point,
    diameter: u32,
    start: Angle,
    sweep: Angle,
    style: &DrawStyle<T::Color>,
) {
    let r = diameter as f32 / 2.0;
    let center = Point::new(
        top_left.x + (diameter / 2) as i32,
        top_left.y + (diameter / 2) as i32,
    );
    let mut points = vec![center];
    points.extend(arc_points(center, r, r, start, sweep));
    polygon(cx, &points, style);
}

/// An ellipse inscribed in `bounding_box` — closed, so it fills.
pub fn ellipse<T: Blitter>(
    cx: &mut RasterCtx<'_, T>,
    bounding_box: Rect,
    style: &DrawStyle<T::Color>,
) {
    let rx = bounding_box.size.width as f32 / 2.0;
    let ry = bounding_box.size.height as f32 / 2.0;
    let center = Point::new(
        bounding_box.top_left.x + (bounding_box.size.width / 2) as i32,
        bounding_box.top_left.y + (bounding_box.size.height / 2) as i32,
    );
    // One point short of a full turn: `polygon` closes it, and a duplicated
    // vertex would be a zero-length edge for the winding test to step over.
    let mut points =
        arc_points(center, rx, ry, Angle::ZERO, Angle::FULL_CIRCLE);
    points.pop();
    polygon(cx, &points, style);
}

/// A rectangle with elliptical corners.
///
/// Walked once — edge, corner arc, edge, corner arc — and handed to [`polygon`],
/// so fill and stroke cannot disagree about where a corner is.
pub fn rounded_rect<T: Blitter>(
    cx: &mut RasterCtx<'_, T>,
    rect: Rect,
    corners: CornerRadii,
    style: &DrawStyle<T::Color>,
) {
    let c = corners.clamp_for(rect.size);
    if c == CornerRadii::default() {
        return self::rect(cx, rect, style);
    }

    let (x, y) = (rect.top_left.x, rect.top_left.y);
    let (w, h) = (rect.size.width as i32, rect.size.height as i32);
    let quarter = Angle::QUARTER_CIRCLE;
    let mut points = Vec::new();

    // Clockwise from the top edge, each corner about its own radius ellipse.
    let corner =
        |points: &mut Vec<Point>, cx_: i32, cy_: i32, r: Size, from: f32| {
            if r.width == 0 || r.height == 0 {
                points.push(Point::new(cx_, cy_));
                return;
            }
            points.extend(arc_points(
                Point::new(cx_, cy_),
                r.width as f32,
                r.height as f32,
                Angle::from_radians(from),
                quarter,
            ));
        };

    let half_pi = core::f32::consts::FRAC_PI_2;
    // Top-right, bottom-right, bottom-left, top-left.
    corner(
        &mut points,
        x + w - c.top_right.width as i32,
        y + c.top_right.height as i32,
        c.top_right,
        -half_pi,
    );
    corner(
        &mut points,
        x + w - c.bottom_right.width as i32,
        y + h - c.bottom_right.height as i32,
        c.bottom_right,
        0.0,
    );
    corner(
        &mut points,
        x + c.bottom_left.width as i32,
        y + h - c.bottom_left.height as i32,
        c.bottom_left,
        half_pi,
    );
    corner(
        &mut points,
        x + c.top_left.width as i32,
        y + c.top_left.height as i32,
        c.top_left,
        core::f32::consts::PI,
    );

    polygon(cx, &points, style);
}

/// Blit an image, one row run at a time.
///
/// # The byte layout is PROVISIONAL
///
/// `ImageRef`'s bytes are read as **premultiplied RGBA8, row-major, four bytes
/// per pixel** — what tiny-skia's `PixmapRef::from_bytes` wants, and the only
/// layout anything in the crate currently assumes.
///
/// TODO (ISSUE-8): they should be the color's own storage — two bytes per pixel
/// for Rgb565, packed bits for `BinaryColor` — which makes a splash screen 7 KiB
/// rather than 225 KiB on a mono panel. Changing it breaks the tiny-skia path
/// and needs an answer about alpha.
///
/// Alpha is un-premultiplied and then **dropped**: the span protocol has no
/// per-pixel alpha, `blend_span` carrying one color and a coverage run.
pub fn image<T: Blitter>(
    cx: &mut RasterCtx<'_, T>,
    image: DrawImage<'_, T::Color>,
) {
    let size = image.size();
    let (w, h) = (size.width as usize, size.height as usize);
    let data = image.data();
    if w == 0 || h == 0 || data.len() < w * h * 4 {
        return;
    }

    let origin = image.position();
    // One allocation, reused per row, which is what `fill_run` wants.
    let mut row: Vec<T::Color> = Vec::with_capacity(w);

    for y in 0..h {
        row.clear();
        for x in 0..w {
            let i = (y * w + x) * 4;
            let (r, g, b, a) = (data[i], data[i + 1], data[i + 2], data[i + 3]);
            let unmul = |c: u8| -> u8 {
                if a == 0 || a == u8::MAX {
                    c
                } else {
                    ((c as u32 * 255) / a as u32).min(255) as u8
                }
            };
            row.push(T::Color::from_rgba(crate::color::Rgba {
                r: unmul(r),
                g: unmul(g),
                b: unmul(b),
                a,
            }));
        }
        cx.run(Span::new(origin.y + y as i32, origin.x, w as u32), &row);
    }
}

// ───────────────────────────────────────────────────────── internals

/// A flattened subpath.
struct SubPath {
    points: Vec<Point>,
    closed: bool,
}

/// Turn a [`Path`] into straight-line subpaths.
fn flatten(path: &Path) -> Vec<SubPath> {
    let mut out: Vec<SubPath> = Vec::new();
    let mut current: Option<SubPath> = None;
    let mut cursor = Point::zero();

    let flush = |current: &mut Option<SubPath>, out: &mut Vec<SubPath>| {
        if let Some(sub) = current.take() {
            if sub.points.len() >= 2 {
                out.push(sub);
            }
        }
    };

    for segment in path.segments() {
        match segment {
            PathSegment::MoveTo(p) => {
                flush(&mut current, &mut out);
                cursor = *p;
                current = Some(SubPath { points: vec![*p], closed: false });
            },
            PathSegment::LineTo(p) => {
                current
                    .get_or_insert_with(|| SubPath {
                        points: vec![cursor],
                        closed: false,
                    })
                    .points
                    .push(*p);
                cursor = *p;
            },
            PathSegment::ArcTo { center, radius, start, sweep } => {
                let r = *radius as f32;
                let points = arc_points(*center, r, r, *start, *sweep);
                if let Some(last) = points.last() {
                    cursor = *last;
                }
                current
                    .get_or_insert_with(|| SubPath {
                        points: Vec::new(),
                        closed: false,
                    })
                    .points
                    .extend(points);
            },
            PathSegment::Close => {
                if let Some(sub) = current.as_mut() {
                    sub.closed = true;
                    cursor = sub.points.first().copied().unwrap_or(cursor);
                }
                flush(&mut current, &mut out);
            },
        }
    }
    flush(&mut current, &mut out);
    out
}

/// Points along an elliptical arc about `center`, inclusive of both ends.
///
/// Angle zero is `+x`; a positive `sweep` runs toward `+y` (clockwise on
/// screen, since `y` grows downward).
fn arc_points(
    center: Point,
    rx: f32,
    ry: f32,
    start: Angle,
    sweep: Angle,
) -> Vec<Point> {
    let sweep_rad = sweep.to_radians();
    let r = rx.max(ry).max(1.0);
    let arc_len = sweep_rad.abs() * r;
    let steps = ((arc_len / FLATTEN_STEP_PX).ceil() as usize).clamp(2, 4096);

    (0..=steps)
        .map(|i| {
            let t = start.to_radians() + sweep_rad * (i as f32 / steps as f32);
            Point::new_rounded(
                center.x as f32 + rx * t.cos(),
                center.y as f32 + ry * t.sin(),
            )
        })
        .collect()
}

fn fill_polygon<T: Blitter>(
    cx: &mut RasterCtx<'_, T>,
    points: &[Point],
    color: T::Color,
) {
    if points.len() < 3 {
        return;
    }
    let Some(bounds) = crate::primitives::polygon::bounds_of(points) else {
        return;
    };
    let bounds = bounds.intersection(&cx.clip());
    if bounds.is_zero_sized() {
        return;
    }

    let end_x = bounds.top_left.x + bounds.size.width as i32;
    for y in bounds.rows() {
        let mut run: Option<i32> = None;
        for x in bounds.columns() {
            if crate::primitives::polygon::contains(points, Point::new(x, y)) {
                run.get_or_insert(x);
            } else if let Some(start) = run.take() {
                cx.span(Span::new(y, start, (x - start) as u32), color);
            }
        }
        if let Some(start) = run {
            cx.span(Span::new(y, start, (end_x - start) as u32), color);
        }
    }
}

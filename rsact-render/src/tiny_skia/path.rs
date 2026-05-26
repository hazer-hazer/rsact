use crate::{
    geometry::{Angle, Point, Rect, Size},
    prelude::CornerRadii,
};

pub const KAPPA: f32 = 0.5; // (4/3) * tan(pi/8)

pub trait PathBuilderExt {
    fn arc(
        &mut self,
        center: Point,
        radius: u32,
        start: Angle,
        sweep: Angle,
    ) -> &mut Self;

    // /// Draw a corner of a rounded rectangle. The start point is where the curve starts (for example the end of the top left corner), the length is either the width or height of the corner (depending on the axis), and the radius is the size of the corner. The drawing is clockwise, meaning that top right corner is defined by width, bottom right by height, bottom left by width, and top left by height.
    // /// The [`radius`] is expected to be pre-clamped to the size of the corner, so that it doesn't exceed the size of the rectangle.
    // fn corner(&mut self, start: Point, length: u32, radius: Size) -> &mut Self;

    fn arc_to(&mut self, from: Point, to: Point, radii: Size) -> &mut Self;

    fn rounded_rect(&mut self, rect: Rect, corners: CornerRadii) -> &mut Self;
}

impl PathBuilderExt for tiny_skia::PathBuilder {
    // // TODO: Fix how arc is drawn, seems to be inaccurate approximation of a circle for me.
    // // TODO: Normalize angles?
    fn arc(
        &mut self,
        center: Point,
        radius: u32,
        start: Angle,
        sweep: Angle,
    ) -> &mut Self {
        if sweep.is_zero() {
            return self;
        }

        let mut current_angle = start.radians;
        let direction = sweep.sign();

        const MAX_SEGMENT_ANGLE: f32 = core::f32::consts::FRAC_PI_2;

        let radius = radius as f32;
        let sweep_abs = sweep.radians.abs();
        let num_segments = (sweep_abs / MAX_SEGMENT_ANGLE).ceil() as u32;
        let segment_sweep = sweep_abs / num_segments as f32;
        let alpha = 4.0 / 3.0 * (segment_sweep / 4.0).tan();

        // TODO: Optimize: re-use first sin/cos
        let (sin_start, cos_start) = start.radians.sin_cos();
        let start_p = tiny_skia::Point::from_xy(
            center.x as f32 + radius * cos_start,
            center.y as f32 + radius * sin_start,
        );
        self.move_to(start_p.x, start_p.y);

        for i in 1..=num_segments {
            let next_angle =
                start.radians + direction * (i as f32 * segment_sweep);
            let (sin_a, cos_a) = current_angle.sin_cos();
            let (sin_b, cos_b) = next_angle.sin_cos();

            let tng_start = (-sin_a, cos_a);
            let tng_end = (-sin_b, cos_b);

            let p1 = tiny_skia::Point::from_xy(
                center.x as f32 + radius * cos_a,
                center.y as f32 + radius * sin_a,
            );
            let p2 = tiny_skia::Point::from_xy(
                center.x as f32 + radius * cos_b,
                center.y as f32 + radius * sin_b,
            );
            let c1 = tiny_skia::Point::from_xy(
                p1.x + alpha * radius * tng_start.0,
                p1.y + alpha * radius * tng_start.1,
            );
            let c2 = tiny_skia::Point::from_xy(
                p2.x - alpha * radius * tng_end.0,
                p2.y - alpha * radius * tng_end.1,
            );

            self.cubic_to(c1.x, c1.y, c2.x, c2.y, p2.x, p2.y);

            current_angle = next_angle;
        }

        self
    }

    fn arc_to(&mut self, start: Point, end: Point, radii: Size) -> &mut Self {
        let delta = end - start;
        let quad = delta.x * delta.y;
        let sign_x = delta.x.signum();
        let sign_y = delta.y.signum();

        let start = tiny_skia::Point::from_xy(start.x as f32, start.y as f32);
        self.move_to(start.x, start.y);

        let radii = Into::<Size<f32>>::into(radii)
            * Size::new(sign_x as f32, sign_y as f32)
            * KAPPA;

        let (cp1, cp2) = if quad > 0 {
            (
                tiny_skia::Point::from_xy(start.x + radii.width, start.y),
                tiny_skia::Point::from_xy(start.x, start.y + radii.height),
            )
        } else {
            (
                tiny_skia::Point::from_xy(start.x, start.y + radii.height),
                tiny_skia::Point::from_xy(start.x + radii.width, start.y),
            )
        };

        self.cubic_to(cp1.x, cp1.y, cp2.x, cp2.y, end.x as f32, end.y as f32);

        self
    }

    fn rounded_rect(&mut self, rect: Rect, corners: CornerRadii) -> &mut Self {
        let x = rect.top_left.x as f32;
        let y = rect.top_left.y as f32;
        let w = rect.size.width as f32;
        let h = rect.size.height as f32;
        let right = x + w;
        let bottom = y + h;

        let tl = corners.top_left;
        let tr = corners.top_right;
        let br = corners.bottom_right;
        let bl = corners.bottom_left;

        // Top right
        let tr_x = tr.width as f32;
        let tr_y = tr.height as f32;
        let tr_end_x = right;
        let tr_end_y = y + tr_y;

        let start_x = x + tl.width as f32;
        self.move_to(start_x, y);

        if w - tl.width as f32 - tr.width as f32 > 0.0 {
            self.line_to(right - tr_x, y);
        }

        if !tr.is_zero() {
            let cp1 = tiny_skia::Point::from_xy(right - tr_x * KAPPA, y);
            let cp2 = tiny_skia::Point::from_xy(right, y + tr_y * KAPPA);

            self.cubic_to(cp1.x, cp1.y, cp2.x, cp2.y, tr_end_x, tr_end_y);
        } else {
            self.line_to(tr_end_x, tr_end_y);
        }

        // Bottom right
        let br_x = br.width as f32;
        let br_y = br.height as f32;
        let br_end_x = right - br_x;
        let br_end_y = bottom;
        self.line_to(right, bottom - br_y);

        if !br.is_zero() {
            let cp1 = tiny_skia::Point::from_xy(right, bottom - br_y * KAPPA);
            let cp2 = tiny_skia::Point::from_xy(right - br_x * KAPPA, bottom);

            self.cubic_to(cp1.x, cp1.y, cp2.x, cp2.y, br_end_x, br_end_y);
        } else {
            self.line_to(br_end_x, br_end_y);
        }

        // Bottom left
        let bl_x = bl.width as f32;
        let bl_y = bl.height as f32;
        let bl_end_x = x;
        let bl_end_y = bottom - bl_y;
        self.line_to(x + bl_x, bottom);

        if !bl.is_zero() {
            let cp1 = tiny_skia::Point::from_xy(x + bl_x * KAPPA, bottom);
            let cp2 = tiny_skia::Point::from_xy(x, bottom - bl_y * KAPPA);

            self.cubic_to(cp1.x, cp1.y, cp2.x, cp2.y, bl_end_x, bl_end_y);
        } else {
            self.line_to(bl_end_x, bl_end_y);
        }

        // Top left
        let tl_x = tl.width as f32;
        let tl_y = tl.height as f32;
        let tl_end_x = x + tl_x;
        let tl_end_y = y;
        self.line_to(x, y + tl_y);

        if !tl.is_zero() {
            let cp1 = tiny_skia::Point::from_xy(x, y + tl_y * KAPPA);
            let cp2 = tiny_skia::Point::from_xy(x + tl_x * KAPPA, y);

            self.cubic_to(cp1.x, cp1.y, cp2.x, cp2.y, tl_end_x, tl_end_y);
        } else {
            self.line_to(tl_end_x, tl_end_y);
        }

        self
    }

    // fn corner(&mut self, start: Point, length: u32, radius: Size) -> &mut Self {
    // }
}

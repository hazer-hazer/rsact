use crate::{
    color::Color,
    eg::{framebuf::PackedColor, primitives::EgPrimitive},
    geometry::{Point, PointExt as _, Size},
    output::pixel::Pixel,
    primitives::{circle::Circle, ellipse::Ellipse},
    renderer::{AntiAliasingDisabled, AntiAliasingEnabled, RenderResult},
    style::StrokeAlignment,
};
use embedded_graphics::{pixelcolor::PixelColor, primitives::StyledDrawable};

impl<C: Color + PixelColor + PackedColor> EgPrimitive<C> for Ellipse {
    fn draw(
        &self,
        renderer: &mut crate::prelude::EGRenderer<C, AntiAliasingDisabled>,
        style: crate::prelude::DrawStyle<C>,
    ) -> RenderResult {
        embedded_graphics::primitives::Ellipse::new(
            self.top_left.into(),
            self.size.into(),
        )
        .draw_styled(&style.into_primitive_style(), renderer)
    }

    fn draw_aa(
        &self,
        renderer: &mut crate::prelude::EGRenderer<C, AntiAliasingEnabled>,
        style: crate::prelude::DrawStyle<C>,
    ) -> RenderResult {
        if self.size.width == self.size.height {
            return Circle::new(self.top_left.into(), self.size.width)
                .draw_aa(renderer, style);
        }

        // TODO: StrokeAlignment

        // FIXME: Ellipse looks bad because of issues of Xiaolin Wu thick stroke
        // drawing.

        let center = self.top_left
            + Point::new(self.size.width as i32, self.size.height as i32) / 2;

        let r = self.size.map(|axis| axis.div_ceil(2));
        // Note: Xiaolin Wu's algorithm draws line at center of the radius, so
        // think about it already being centered on ellipse line
        let stroke_size = Size::new_equal(style.stroke_width);
        let half_stroke_size = Size::new_equal(style.stroke_width / 2);
        // let half_ceil_stroke_size =
        //     Size::new_equal(style.stroke_width.div_ceil(2));
        let (r_stroke, _r_fill) = match style.stroke_alignment {
            StrokeAlignment::Inside => (r - stroke_size, r - half_stroke_size),
            StrokeAlignment::Center => (r, r - stroke_size),
            StrokeAlignment::Outside => {
                (r + half_stroke_size, r + half_stroke_size)
            },
        };

        let r_stroke_sq = r_stroke.map(|r| r.pow(2));
        let stroke_offset_x = style.stroke_width as i32 / 2;
        let stroke_offset_y = style.stroke_width as i32 / 2;

        let mut set_point = |delta: Point, color: C, blend: f32| {
            delta.each_mirror().try_for_each(|delta| {
                renderer.pixel_alpha(
                    // TODO: Remove unwrap
                    Pixel(center + delta, color),
                    blend,
                )
            })
        };

        // Can avoid float usage?
        let quart = (r_stroke_sq.width as f32
            / (r_stroke_sq.width as f32 + r_stroke_sq.height as f32).sqrt())
        .round() as i32;

        for x in 0..=quart {
            let y = r_stroke.height as f32
                * (1.0 - x.pow(2) as f32 / r_stroke_sq.width as f32).sqrt();

            // TODO: Fill antialiasing
            if let Some(fill_color) = style.fill {
                for y in 0..=y.floor() as i32 {
                    set_point(Point::new(x, y), fill_color, 1.0)?;
                }

                if style.stroke.is_none() {
                    set_point(
                        Point::new(x, y.floor() as i32 + 1),
                        fill_color,
                        y.fract(),
                    )?;
                }
            }

            if let (stroke_width @ 1.., Some(stroke_color)) =
                (style.stroke_width, style.stroke)
            {
                let alpha = y.fract();

                let point = Point::new(x, y.floor() as i32 - stroke_offset_y);
                set_point(point, stroke_color, 1.0 - alpha)?;
                for w in 1..stroke_width as i32 {
                    set_point(point.add_y(w), stroke_color, 1.0)?;
                }
                set_point(
                    point.add_y(stroke_width as i32),
                    stroke_color,
                    alpha,
                )?;
            }
        }

        // let quart = (r_stroke_sq.height as f32
        //     / (r_stroke_sq.width as f32 + r_stroke_sq.height as f32).sqrt())
        // .round() as i32;

        for y in 0..=r_stroke.height as i32 {
            let x = r_stroke.width as f32
                * (1.0 - y.pow(2) as f32 / r_stroke_sq.height as f32).sqrt();

            // TODO: Fix fill with stroke overlap
            if let Some(fill_color) = style.fill {
                for x in 0..=x.floor() as i32 {
                    set_point(Point::new(x, y), fill_color, 1.0)?;
                }

                if style.stroke.is_none() {
                    set_point(
                        Point::new(x.floor() as i32 + 1, y),
                        fill_color,
                        x.fract(),
                    )?;
                }
            }

            if let (stroke_width @ 1.., Some(stroke_color)) =
                (style.stroke_width, style.stroke)
            {
                let alpha = x.fract();

                let point = Point::new(x.floor() as i32 - stroke_offset_x, y);
                set_point(point, stroke_color, 1.0 - alpha)?;
                for w in 1..stroke_width as i32 {
                    set_point(point.add_x(w), stroke_color, 1.0)?;
                }
                set_point(
                    point.add_x(stroke_width as i32),
                    stroke_color,
                    alpha,
                )?;
            }
        }

        Ok(())
    }
}

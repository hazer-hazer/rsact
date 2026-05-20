use crate::{
    color::Color,
    eg::alpha::{AlphaDrawTarget, StyledAlphaDrawable},
    geometry::PointExt as _,
    primitives::line::Line, renderer::RenderResult,
};
use embedded_graphics::{
    Pixel,
    geometry::Point as EgPoint,
    prelude::{Angle, Dimensions, Primitive, Transform},
    primitives::{PrimitiveStyle, StyledDrawable},
};
use num::Float as _;

impl Dimensions for Line {
    fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
        embedded_graphics::primitives::Rectangle::new(
            self.from.into(),
            embedded_graphics::geometry::Size::new(
                (self.to.x - self.from.x).abs() as u32,
                (self.to.y - self.from.y).abs() as u32,
            ),
        )
    }
}

impl Primitive for Line {}

impl Transform for Line {
    fn translate(&self, by: EgPoint) -> Self {
        let mut new = *self;
        new.from += by.into();
        new.to += by.into();
        new
    }

    fn translate_mut(&mut self, by: EgPoint) -> &mut Self {
        self.from += by.into();
        self.to += by.into();
        self
    }
}

impl<C: Color + embedded_graphics::prelude::PixelColor>
    StyledDrawable<PrimitiveStyle<C>> for Line
{
    type Color = C;
    type Output = ();

    fn draw_styled<D>(
        &self,
        style: &PrimitiveStyle<C>,
        target: &mut D,
    ) -> Result<Self::Output, D::Error>
    where
        D: embedded_graphics::prelude::DrawTarget<Color = Self::Color>,
    {
        embedded_graphics::primitives::Line::new(
            self.from.into(),
            self.to.into(),
        )
        .draw_styled(style, target)
    }
}

impl<C: Color + embedded_graphics::prelude::PixelColor>
    StyledAlphaDrawable<PrimitiveStyle<C>> for Line
{
    type Color = C;
    type Output = ();

    fn draw_styled_alpha<D>(
        &self,
        style: &PrimitiveStyle<C>,
        target: &mut D,
    ) -> RenderResult
    where
        D: AlphaDrawTarget<Color = Self::Color>,
    {
        if style.stroke_color.is_none() || style.stroke_width == 0 {
            return Ok(());
        }

        let mut start = self.from;
        let mut end = self.to;
        let mut draw_pixel = |point, blend| {
            target.pixel_alpha(Pixel(point, style.stroke_color.unwrap()), blend)
        };

        let steep = (end.y - start.y).abs() > (end.x - start.x).abs();

        start = start.swap_axes_if(steep);
        end = end.swap_axes_if(steep);

        if start.x > end.x {
            core::mem::swap(&mut start, &mut end);
        }

        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let gradient = if dx > 0 { dy as f32 / dx as f32 } else { 1.0 };

        let width = style.stroke_width as i32;
        let w = width as f32 * (1.0 + gradient.powi(2)).sqrt();
        let draw_width = w.round() as i32;

        let x_end = start.x as f32;
        let y_end = start.y as f32 - (w - 1.0) * 0.5;
        let x_gap = 0.5;
        let x_pixel1 = x_end;
        let y_pixel1 = y_end.floor();
        let fpart = y_end.fract();
        let rfpart = 1.0 - fpart;

        let point = EgPoint::new(x_pixel1 as i32, y_pixel1 as i32);
        draw_pixel(point.swap_axes_if(steep), rfpart * x_gap)?;
        for w in 1..draw_width {
            draw_pixel(point.add_y(w).swap_axes_if(steep), 1.0)?;
        }
        draw_pixel(point.add_y(draw_width).swap_axes_if(steep), fpart * x_gap)?;

        let mut inter_y = y_end + gradient;

        let x_end = end.x as f32;
        let y_end = end.y as f32 - (w - 1.0) * 0.5;
        let x_gap = 0.5;
        let x_pixel2 = x_end;
        let y_pixel2 = y_end.floor();
        let fpart = y_end.fract();
        let rfpart = 1.0 - fpart;

        let point = EgPoint::new(x_pixel2 as i32, y_pixel2 as i32);
        draw_pixel(point.swap_axes_if(steep), rfpart * x_gap)?;
        for w in 1..draw_width {
            draw_pixel(point.add_y(w).swap_axes_if(steep), 1.0)?;
        }
        draw_pixel(point.add_y(draw_width).swap_axes_if(steep), fpart * x_gap)?;

        for x in x_pixel1.round() as i32 + 1..x_pixel2.round() as i32 {
            let fpart = inter_y.fract();
            let rfpart = 1.0 - fpart;
            let y = inter_y.floor() as i32;

            let point = EgPoint::new(x, y);
            draw_pixel(point.swap_axes_if(steep), rfpart)?;
            for w in 1..draw_width {
                draw_pixel(point.add_y(w).swap_axes_if(steep), 1.0)?;
            }
            draw_pixel(point.add_y(draw_width).swap_axes_if(steep), fpart)?;
            inter_y += gradient;
        }

        Ok(())
    }
}

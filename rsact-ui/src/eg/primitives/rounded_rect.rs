use crate::{
    eg::alpha::{AlphaDrawTarget, StyledAlphaDrawable},
    geometry::*,
    prelude::Color,
    render::primitives::{line::Line, rounded_rect::RoundedRect},
    utils::min_max_range_incl,
};
use embedded_graphics::{
    Pixel,
    prelude::{Dimensions, Primitive, Transform},
    primitives::{PrimitiveStyle, StyledDrawable},
};
use num::Float as _;

impl Dimensions for RoundedRect {
    fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
        self.rect.into()
    }
}

impl Primitive for RoundedRect {}

impl Transform for RoundedRect {
    fn translate(&self, by: embedded_graphics::prelude::Point) -> Self {
        let our_by: Point = by.into();
        Self::new(self.rect.translate(our_by), self.corners)
    }

    fn translate_mut(
        &mut self,
        by: embedded_graphics::prelude::Point,
    ) -> &mut Self {
        let our_by: Point = by.into();
        self.rect.translate_mut(our_by);
        self
    }
}

impl<C: Color + embedded_graphics::prelude::PixelColor>
    StyledDrawable<PrimitiveStyle<C>> for RoundedRect
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
        embedded_graphics::primitives::RoundedRectangle::new(
            self.rect.into(),
            self.corners.into(),
        )
        .draw_styled(style, target)
    }
}

impl<C: Color + embedded_graphics::prelude::PixelColor>
    StyledAlphaDrawable<PrimitiveStyle<C>> for RoundedRect
{
    type Color = C;
    type Output = ();

    fn draw_styled_alpha<D>(
        &self,
        style: &PrimitiveStyle<C>,
        target: &mut D,
    ) -> crate::prelude::RenderResult
    where
        D: AlphaDrawTarget<Color = Self::Color>,
    {
        let corner_radii = self.corners;

        // TODO: Bad ellipse drawing with stroke_width > 1

        // There must be a better way to fill this cross
        if let Some(fill_color) = style.fill_color {
            let width = self.rect.size.width;
            let height = self.rect.size.height;
            let top = corner_radii.top_left.width
                ..width - corner_radii.top_right.width;
            let right = corner_radii.top_right.height
                ..height - corner_radii.bottom_right.height;
            let bottom = corner_radii.bottom_left.width
                ..width - corner_radii.bottom_right.width;
            let left = corner_radii.top_left.height
                ..height - corner_radii.bottom_left.height;

            for w in 0..self.rect.size.width {
                for h in 0..self.rect.size.height {
                    let top = top.contains(&w);
                    let right = right.contains(&h);
                    let bottom = bottom.contains(&w);
                    let left = left.contains(&h);

                    if [top, right, bottom, left].iter().filter(|c| **c).count()
                        > 1
                    {
                        target.pixel_alpha(
                            Pixel(
                                (self.rect.top_left
                                    + Point::new(w as i32, h as i32))
                                .into(),
                                fill_color,
                            ),
                            1.0,
                        )?;
                    }
                }
            }
        }

        [
            (AnchorPoint::TopLeft, AnchorPoint::TopRight, Axis::X),
            (AnchorPoint::TopRight, AnchorPoint::BottomRight, Axis::Y),
            (AnchorPoint::BottomRight, AnchorPoint::BottomLeft, Axis::X),
            (AnchorPoint::BottomLeft, AnchorPoint::TopLeft, Axis::Y),
        ]
        .into_iter()
        .try_for_each(|(start, end, axis)| {
            let start_v: UnitV2 = start.into();
            let end_v: UnitV2 = end.into();

            let start_corner_radius = corner_radii.by_unit_v(start_v).unwrap();
            let start_anchor_point = self.rect.anchor_point(start);

            Line::new(
                start_anchor_point.add_main(
                    axis,
                    start_corner_radius.main(axis) as i32
                        * start_v.main(axis) as i32
                        * -1,
                ),
                self.rect.anchor_point(end).add_main(
                    axis,
                    corner_radii.by_unit_v(end_v).unwrap().main(axis) as i32
                        * end_v.main(axis) as i32
                        * -1,
                ),
            )
            .draw_styled(style, target)
            .ok()
            .unwrap();

            if start_corner_radius.width == 0 || start_corner_radius.height == 0
            {
                return Ok(());
            }

            let w = style.stroke_width as i32;
            let r: Point = start_corner_radius.try_into().unwrap();
            let r_sq = r.map(|r| r.pow(2));
            let center = start_anchor_point - r * start_v;
            let (x_v, y_v) = start_v.destruct();
            let stroke_offset = w / 2;

            for x in min_max_range_incl(0, x_v * r.x) {
                let y =
                    r.y as f32 * (1.0 - x.pow(2) as f32 / r_sq.x as f32).sqrt();

                if let Some(fill_color) = style.fill_color {
                    for h in 0..=y.floor() as i32 {
                        target.pixel_alpha(
                            Pixel(
                                (center + Point::new(x, h * y_v)).into(),
                                fill_color,
                            ),
                            1.0,
                        )?;
                    }

                    if style.stroke_color.is_none() {
                        target.pixel_alpha(
                            Pixel(
                                (center
                                    + Point::new(
                                        x,
                                        (y.floor() as i32 + 1) * y_v,
                                    ))
                                .into(),
                                fill_color,
                            ),
                            y.fract(),
                        )?;
                    }
                }

                if let Some(stroke_color) = style.stroke_color {
                    let point = center
                        + Point::new(
                            x,
                            (y.floor() as i32 - stroke_offset) * y_v,
                        );

                    target.pixel_alpha(
                        Pixel(point.into(), stroke_color),
                        1.0 - y.fract(),
                    )?;

                    for y in 1..w {
                        target.pixel_alpha(
                            Pixel(point.add_y(y * y_v).into(), stroke_color),
                            1.0,
                        )?;
                    }

                    target.pixel_alpha(
                        Pixel(point.add_y(w * y_v).into(), stroke_color),
                        y.fract(),
                    )?;
                }
            }

            for y in min_max_range_incl(0, y_v * r.y) {
                let x =
                    r.x as f32 * (1.0 - y.pow(2) as f32 / r_sq.y as f32).sqrt();

                if let Some(fill_color) = style.fill_color {
                    for w in 0..=x.floor() as i32 {
                        target.pixel_alpha(
                            Pixel(
                                (center + Point::new(w * y_v, y)).into(),
                                fill_color,
                            ),
                            1.0,
                        )?;
                    }

                    if style.stroke_color.is_none() {
                        target.pixel_alpha(
                            Pixel(
                                (center
                                    + Point::new(
                                        (x.floor() as i32 + 1) * x_v,
                                        y,
                                    ))
                                .into(),
                                fill_color,
                            ),
                            x.fract(),
                        )?;
                    }
                }

                if let Some(stroke_color) = style.stroke_color {
                    let point = center
                        + Point::new(
                            (x.floor() as i32 - stroke_offset) * x_v,
                            y,
                        );

                    target.pixel_alpha(
                        Pixel(point.into(), stroke_color),
                        1.0 - x.fract(),
                    )?;

                    for x in 1..w {
                        target.pixel_alpha(
                            Pixel(point.add_x(x * x_v).into(), stroke_color),
                            1.0,
                        )?;
                    }

                    target.pixel_alpha(
                        Pixel(point.add_x(w * x_v).into(), stroke_color),
                        x.fract(),
                    )?;
                }
            }

            Ok(())
        })
    }
}

use core::mem::swap;

use crate::layout::size::PointExt;

use super::color::Color;
use embedded_graphics::{
    prelude::{Point, Primitive},
    primitives::{PrimitiveStyle, PrimitiveStyleBuilder},
    Drawable, Pixel,
};
use num::Zero;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum LineAlgo {
    Bresenham,
    XiaolinWu,
    Midpoint,
}

pub struct LineStyle<C: Color> {
    color: Option<C>,
    algo: LineAlgo,
}

impl<C: Color> LineStyle<C> {
    fn should_draw(&self) -> bool {
        self.color.is_some()
    }

    fn as_eg(&self) -> PrimitiveStyle<C> {
        let style = PrimitiveStyleBuilder::new();

        if let Some(color) = self.color {
            style.stroke_color(color)
        } else {
            style
        }
        .build()
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BlendingPoint(Point, f32);

// TODO: Implement [Midpoint line algorithm](https://www.mat.univie.ac.at/%7Ekriegl/Skripten/CG/node25.html)

/// Generic line structure used by [crate::render::`Renderer`]'s.
/// The line that can be drawn using [Xiaolin Wu's algorithm](https://en.wikipedia.org/wiki/Xiaolin_Wu%27s_line_algorithm) that does anti-aliasing.
/// Less efficient than Bresenham's algorithm embedded-graphics uses, but much better looking results.
pub struct Line<C: Color> {
    start: Point,
    end: Point,
    style: LineStyle<C>,
}

// impl<C: Color> Drawable for Line<C> {
//     type Color = C;
//     type Output = ();

//     fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
//     where
//         D: embedded_graphics::prelude::DrawTarget<Color = Self::Color>,
//     {
//         if !self.style.should_draw() {
//             return Ok(());
//         }

//         match self.style.algo {
//             LineAlgo::Bresenham => {
//                 embedded_graphics::primitives::Line::new(self.start, self.end)
//                     .into_styled(self.style.as_eg())
//                     .draw(target)
//             },
//             LineAlgo::XiaolinWu => todo!(),
//             LineAlgo::Midpoint => todo!(),
//         }
//     }
// }

// pub struct XiaolinWuPoints {
//     steep: bool,
//     gradient: f32,
//     x: f32,
//     y: f32,
//     end_x: f32,
//     lower: bool,
// }

// impl XiaolinWuPoints {
//     pub fn new(mut start: Point, mut end: Point) -> Self {
//         // The line is steep if the difference Xs is less than difference between Ys
//         let steep = (end.y - start.y).abs() > (end.x - start.x).abs();

//         if steep {
//             start = start.swap_axis();
//             end = end.swap_axis();
//         }

//         if start.x > end.x {
//             swap(&mut start, &mut end);
//         }

//         let dx = end.x - start.x;
//         let gradient =
//             if dx == 0 { 1.0 } else { (end.y - start.y) as f32 / dx as f32 };

//         Self {
//             steep,
//             gradient,
//             x: start.x as f32,
//             y: start.y as f32,
//             end_x: end.x as f32,
//             lower: false,
//         }
//     }
// }

// impl Iterator for XiaolinWuPoints {
//     type Item = BlendingPoint;

//     fn next(&mut self) -> Option<Self::Item> {
/*

if self.x <= self.end_x {
    // get the fractional part of y
    let fpart = self.y - self.y.floor();

    // Calculate the integer value of y
    let mut y = O::cast(self.y);
    if self.lower {
        y += O::one();
    }

    // Get the point
    let point = if self.steep { (y, self.x) } else { (self.x, y) };

    if self.lower {
        // Return the lower point
        self.lower = false;
        self.x += O::one();
        self.y += self.gradient;
        Some((point, fpart))
    } else {
        if fpart > I::zero() {
            // Set to return the lower point if the fractional part is > 0
            self.lower = true;
        } else {
            // Otherwise move on
            self.x += O::one();
            self.y += self.gradient;
        }

        // Return the remainer of the fractional part
        Some((point, I::one() - fpart))
    }
} else {
    None
}
 */
//         if self.x <= self.end_x {
//             let fpart = self.y - self.y.floor();

//             let mut y = self.y;
//             if self.lower {
//                 y += 1.0;
//             }

//             let point = if self.steep { (y, self.x) } else { (self.x, y) };

//             if self.lower {
//                 self.lower = false;
//                 self.x += 1.0;
//                 self.y += self.gradient;
//                 Some((point, fpart))
//             } else {
//                 if fpart > 0.0 {
//                     self.lower = true;
//                 } else {
//                     self.x += 1.0;
//                     self.y += self.gradient;
//                 }

//                 Some((point, 1 - fpart))
//             }
//         } else {
//             None
//         }
//     }
// }

// pub fn xiaolin_wu(
//     mut start: Point,
//     mut end: Point,
//     width: u32,
//     mut draw_pixel: impl FnMut(Point, f32),
// ) {
//     let mut x0 = start.x;
//     let mut y0 = start.y;
//     let mut x1 = end.x;
//     let mut y1 = end.y;

//     // Swap if line is steep
//     let steep = (y1 - y0).abs() > (x1 - x0).abs();
//     if steep {
//         core::mem::swap(&mut x0, &mut y0);
//         core::mem::swap(&mut x1, &mut y1);
//     }

//     // Swap endpoints if necessary
//     if x0 > x1 {
//         core::mem::swap(&mut x0, &mut x1);
//         core::mem::swap(&mut y0, &mut y1);
//     }

//     let dx = x1 - x0;
//     let dy = y1 - y0;
//     let gradient = if dx > 0 { dy as f32 / dx as f32 } else { 1.0 };

//     let mut w = width as f32;
//     // Adjust width based on gradient
//     w *= (1.0 + gradient * gradient).sqrt();
//     let w = w as i32;

//     // Draw a pixel and its width neighbors
//     let mut draw_with_width = |x: i32, y: i32, alpha: f32| {
//         if steep {
//             draw_pixel(Point::new(y, x), alpha);
//             for i in 1..(w as i32) {
//                 draw_pixel(Point::new(y + i, x), 1.0);
//             }
//         } else {
//             draw_pixel(Point::new(x, y), alpha);
//             for i in 1..(w as i32) {
//                 draw_pixel(Point::new(x, y + i), 1.0);
//             }
//         }
//     };

//     // Draw endpoints
//     let mut plot_endpoint = |x: i32, y: i32, offset: f32| {
//         let xend = x;
//         let yend =
//             y as f32 - (w as f32 - 1.0) * 0.5 + gradient * (xend - x) as f32;
//         let xgap = 1.0 - (x as f32 + 0.5 - xend as f32);
//         let yfloor = yend as i32;
//         let fpart = yend - yfloor as f32;
//         let rfpart = 1.0 - fpart;

//         draw_with_width(xend, yfloor, rfpart * xgap);
//         draw_with_width(xend, yfloor + w as i32, fpart * xgap);

//         yend + gradient
//     };

//     // Plot first endpoint and calculate first intersection
//     let mut intery = plot_endpoint(x0, y0, 1.0);

//     // Plot second endpoint
//     plot_endpoint(x1, y1, 1.0);

//     // Main loop to draw the line between endpoints
//     for x in (x0 as i32 + 1)..(x1 as i32) {
//         let fpart = intery - intery.floor();
//         let rfpart = 1.0 - fpart;
//         let y = intery as i32;
//         draw_with_width(x, y, rfpart);
//         draw_with_width(x, y + w as i32, fpart);
//         intery += gradient;
//     }
// }

// pub fn xiaolin_wu(
//     mut start: Point,
//     mut end: Point,
//     width: u32,
//     mut draw: impl FnMut(Point, f32),
// ) {
//     let steep = (end.y - start.y).abs() > (end.x - start.x).abs();

//     if steep {
//         start = start.swap_axis();
//         end = end.swap_axis();
//     }

//     if start.x > end.x {
//         swap(&mut start, &mut end);
//     }

//     let dx = end.x - start.x;
//     let dy = end.y - start.y;
//     let gradient = if dx > 0 { dy as f32 / dx as f32 } else { 1.0 };

//     let mut width = width as f32;

//     width *= (1.0 + gradient * gradient).sqrt();

//     // First endpoint //
//     let end_x = start.x as f32;
//     let end_y = start.y as f32 - (width - 1.0) * 0.5;

//     let x_gap = 1.0 - (start.x as f32 + 0.5 - end_x);

//     let start_point = Point::new(end_x as i32, end_y as i32);
//     let f_part = end_y.fract();
//     let rf_part = 1.0 - f_part;

//     draw(start_point.swap_axis_if(steep), rf_part * x_gap);

//     for i in 1..width as i32 {
//         draw(start_point.add_y(i).swap_axis_if(steep), 1.0);
//     }

//     draw(start_point.add_y(width as i32).swap_axis_if(steep), f_part * x_gap);

//     //
//     let mut inter_y = end_y + gradient;

//     // Second endpoint //
//     let end_x = end.x as f32;
//     let end_y = end.y as f32 - (width - 1.0) * 0.5;

//     let gap_x = 1.0 - (end_x.fract() + 0.5);

//     let end_point = Point::new(end_x as i32, end_y as i32);
//     let f_part = end_y.fract();
//     let rf_part = 1.0 - f_part;

//     draw(end_point, rf_part * gap_x);

//     for i in 1..width as i32 {
//         draw(end_point.add_y(i).swap_axis_if(steep), 1.0);
//     }

//     draw(end_point.add_y(width as i32).swap_axis_if(steep), f_part * gap_x);

//     for x in start_point.x + 1..end_point.x {
//         let f_part = inter_y.fract();
//         let rf_part = 1.0 - rf_part;

//         let y = inter_y as i32;

//         let point = Point::new(x, y);
//         draw(point.swap_axis_if(steep), rf_part);

//         for i in 1..width as i32 {
//             draw(point.add_y(i).swap_axis_if(steep), 1.0);
//         }

//         draw(point.add_y(width as i32).swap_axis(), f_part);

//         inter_y += gradient;
//     }
// }
// pub fn xiaolin_wu<F>(
//     mut start: Point,
//     mut p1: Point,
//     mut w: u32,
//     mut draw_pixel: F,
// ) where
//     F: FnMut(Point, f32), // Closure to draw pixels
// {
//     // Swap if line is steep
//     let steep = (end.y - start.y).abs() > (p1.x - start.x).abs();
//     if steep {
//         core::mem::swap(&mut start.x, &mut start.y);
//         core::mem::swap(&mut p1.x, &mut end.y);
//     }

//     // Swap endpoints if necessary
//     if start.x > p1.x {
//         core::mem::swap(&mut start, &mut p1);
//     }

//     let dx = (p1.x - start.x) as f32;
//     let dy = (end.y - start.y) as f32;
//     let gradient = if dx > 0.0 { dy / dx } else { 1.0 };

//     let mut w = w as f32;
//     // Adjust width based on gradient (rotated width)
//     w *= (1.0 + gradient * gradient).sqrt();

//     // Helper to draw a pixel and its width neighbors
//     let mut draw_with_width = |x: i32, y: i32, alpha: f32| {
//         if steep {
//             draw_pixel(Point::new(y, x), alpha);
//             for i in 1..(w as i32) {
//                 draw_pixel(Point::new(y + i, x), 1.0);
//             }
//         } else {
//             draw_pixel(Point::new(x, y), alpha);
//             for i in 1..(w as i32) {
//                 draw_pixel(Point::new(x, y + i), 1.0);
//             }
//         }
//     };

//     // Interpolate between the fractional parts of endpoints to calculate intensities
//     let interpolate_endpoint = |p: Point| -> (i32, i32, f32, f32) {
//         let xend = p.x;
//         let yend =
//             p.y as f32 - (w - 1.0) * 0.5 + gradient * ((xend - start.x) as f32);
//         let yfloor = yend.floor() as i32;
//         let fpart = yend - yend.floor();
//         let rfpart = 1.0 - fpart;
//         (xend, yfloor, fpart, rfpart)
//     };

//     // Draw first endpoint
//     let (xpxl1, ypxl1, fpart1, rfpart1) = interpolate_endpoint(start);
//     draw_with_width(xpxl1, ypxl1, rfpart1);
//     draw_with_width(xpxl1, ypxl1 + (w as i32), fpart1);

//     let mut intery = (ypxl1 as f32) + gradient; // First y-intersection for main loop

//     // Draw second endpoint
//     let (xpxl2, ypxl2, fpart2, rfpart2) = interpolate_endpoint(p1);
//     draw_with_width(xpxl2, ypxl2, rfpart2);
//     draw_with_width(xpxl2, ypxl2 + (w as i32), fpart2);

//     // Main loop to draw the line between the endpoints
//     for x in (xpxl1 + 1)..xpxl2 {
//         let y = intery.floor() as i32;
//         let fpart = intery - intery.floor();
//         let rfpart = 1.0 - fpart;
//         draw_with_width(x, y, rfpart);
//         draw_with_width(x, y + (w as i32), fpart);
//         intery += gradient;
//     }
// }

// pub fn bresenham_mod<F>(p0: Point, p1: Point, thickness: i32, mut draw_pixel: F)
// where
//     F: FnMut(Point, u8),
// {
//     let p0 = Point::new(p0.x, p0.y * 256);
//     let p1 = Point::new(p1.x, p1.y * 256);

//     // Get the differences in x and y
//     let mut dx = (p1.x - p0.x).abs();
//     let mut dy = (p1.y - p0.y).abs() * 256;

//     // Determine if the line is steep
//     let steep = dy > dx;

//     // If steep, swap x and y
//     let (mut x0, mut y0, mut x1, mut y1) =
//         if steep { (p0.y, p0.x, p1.y, p1.x) } else { (p0.x, p0.y, p1.x, p1.y) };

//     // Ensure that x0 < x1
//     if x0 > x1 {
//         core::mem::swap(&mut x0, &mut x1);
//         core::mem::swap(&mut y0, &mut y1);
//     }

//     // Recalculate differences
//     dx = x1 - x0;
//     dy = (y1 - y0).abs();

//     let mut error = dx / 2; // Initial error value (delta)
//     let y_step = if y0 < y1 { 1 } else { -1 };

//     let mut y = y0;

//     // Loop through each x from x0 to x1 and draw the thick line
//     for x in x0..=x1 {
//         // Instead of drawing just one pixel, we now draw a "band" of pixels
//         for offset in -(thickness / 2)..=(thickness / 2) {
//             let point = Point::new(x, y + offset).swap_axis_if(steep);

//             // draw_pixel(point, 1.0);

//             draw_pixel(
//                 Point::new(point.x, y_step + point.y >> 8),
//                 (y ^ 255) as u8,
//             );
//             // draw_pixel(
//             //     Point::new(point.x, (point.y >> 8)),
//             //     // Point::new(point.x, y_step + (point.y >> 8)),
//             //     (y & 255) as u8,
//             // );
//         }

//         error -= dy;
//         if error < 0 {
//             y += y_step;
//             error += dx;
//         }
//     }
// }

// pub fn xiaolin_wu<F>(start: Point, end: Point, width: u32, mut draw_pixel: F)
// where
//     F: FnMut(Point, u8), // draw_pixel(Point, alpha) where alpha is 0-255
// {
//     let mut dx = (end.x - start.x).abs();
//     let mut dy = (end.y - start.y).abs();

//     let steep = dy > dx;

//     // Swap x and y coordinates if the line is steep
//     let (mut x0, mut y0, mut x1, mut y1) = if steep {
//         (start.y, start.x, end.y, end.x)
//     } else {
//         (start.x, start.y, end.x, end.y)
//     };

//     if x0 > x1 {
//         core::mem::swap(&mut x0, &mut x1);
//         core::mem::swap(&mut y0, &mut y1);
//     }

//     dx = x1 - x0;
//     dy = (y1 - y0).abs();
//     let gradient = if dx == 0 { 1.0 } else { dy as f32 / dx as f32 };

//     // Compute the half-width of the thick line
//     let half_width = (width as f32 - 1.0) / 2.0;

//     // Handle the first endpoint
//     let x_end = x0;
//     let y_end = y0 as f32 + gradient * (x_end as f32 - x0 as f32);
//     let mut x_px1 = x_end;
//     let mut y_px1 = y_end as i32;

//     let x_gap = 1.0 - (x0 as f32 + 0.5).fract();
//     let intery = y_end + gradient; // First y-intersection for the main loop

//     // Draw the first endpoint with anti-aliasing and thickness
//     for offset in -(half_width.ceil() as i32)..=(half_width.ceil() as i32) {
//         let alpha_start =
//             ((1.0 - (y_end + offset as f32).fract()) * x_gap * 255.0) as u8;
//         let alpha_end = ((y_end + offset as f32).fract() * x_gap * 255.0) as u8;

//         if steep {
//             draw_pixel(Point { x: y_px1 + offset, y: x_px1 }, alpha_start);
//             draw_pixel(Point { x: y_px1 + offset + 1, y: x_px1 }, alpha_end);
//         } else {
//             draw_pixel(Point { x: x_px1, y: y_px1 + offset }, alpha_start);
//             draw_pixel(Point { x: x_px1, y: y_px1 + offset + 1 }, alpha_end);
//         }
//     }

//     // Handle the second endpoint
//     let x_end = x1;
//     let y_end = y1 as f32 + gradient * (x_end as f32 - x1 as f32);
//     let mut x_px2 = x_end;
//     let mut y_px2 = y_end as i32;

//     let x_gap = (x1 as f32 + 0.5).fract();
//     for offset in -(half_width.ceil() as i32)..=(half_width.ceil() as i32) {
//         let alpha_start =
//             ((1.0 - (y_end + offset as f32).fract()) * x_gap * 255.0) as u8;
//         let alpha_end = ((y_end + offset as f32).fract() * x_gap * 255.0) as u8;

//         if steep {
//             draw_pixel(Point { x: y_px2 + offset, y: x_px2 }, alpha_start);
//             draw_pixel(Point { x: y_px2 + offset + 1, y: x_px2 }, alpha_end);
//         } else {
//             draw_pixel(Point { x: x_px2, y: y_px2 + offset }, alpha_start);
//             draw_pixel(Point { x: x_px2, y: y_px2 + offset + 1 }, alpha_end);
//         }
//     }

//     // Main loop to draw the line between the endpoints with thickness
//     let mut intery = intery; // Start y-intersection from first pixel
//     for x in (x_px1 + 1)..x_px2 {
//         for offset in -(half_width.ceil() as i32)..=(half_width.ceil() as i32) {
//             let alpha1 =
//                 ((1.0 - (intery + offset as f32).fract()) * 255.0) as u8;
//             let alpha2 = ((intery + offset as f32).fract() * 255.0) as u8;

//             if steep {
//                 draw_pixel(Point { x: intery as i32 + offset, y: x }, alpha1);
//                 draw_pixel(
//                     Point { x: (intery as i32 + offset) + 1, y: x },
//                     alpha2,
//                 );
//             } else {
//                 draw_pixel(Point { x, y: intery as i32 + offset }, alpha1);
//                 draw_pixel(
//                     Point { x, y: (intery as i32 + offset) + 1 },
//                     alpha2,
//                 );
//             }
//         }
//         intery += gradient;
//     }
// }

pub fn xiaolin_wu<F>(start: Point, end: Point, width: u32, mut draw: F)
where
    F: FnMut(Point, f32),
{
    let steep = (end.y - start.y).abs() > (end.x - start.x).abs();

    let mut start = start.swap_axis_if(steep);
    let mut end = end.swap_axis_if(steep);

    if start.x > end.x {
        core::mem::swap(&mut start.x, &mut end.x);
        core::mem::swap(&mut start.y, &mut end.y);
    }

    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let gradient = if dx > 0 { dy as f32 / dx as f32 } else { 1.0 };

    let width = width as f32 * (1.0 + gradient * gradient).sqrt();

    let end_x = start.x as f32;
    let end_y = start.y as f32 - (width - 1.0) * 0.5
        + gradient * (end_x - start.x as f32);
    let x_gap = 1.0 - (start.x as f32 + 0.5 - end_x);
    let x_pixel1 = end_x;
    let y_pixel1 = end_y.floor();
    let fpart = end_y.fract();
    let rfpart = 1.0 - fpart;

    // Draw first endpoint
    let point = Point::new(x_pixel1.round() as i32, y_pixel1.round() as i32);
    draw(point.swap_axis_if(steep), rfpart * x_gap);
    for i in 1..width.round() as i32 {
        draw(point.add_y(i).swap_axis_if(steep), 1.0);
    }
    draw(point.add_y_round(width).swap_axis_if(steep), fpart * x_gap);

    return;

    let mut inter_y = end_y + gradient;

    // Draw second endpoint
    let end_x = end.x as f32;
    let end_y =
        // Note end_x was integer, end.x is integer too, so `gradient * (end_x - end.x)` is always 0
        end.y as f32 - (width - 1.0) * 0.5 + gradient * (end_x - end.x as f32);
    let x_gap = 1.0 - (end.x as f32 + 0.5 - end_x);
    let x_pixel2 = end_x;
    let y_pixel2 = end_y.floor();
    let fpart = end_y.fract();
    let rfpart = 1.0 - fpart;

    let point = Point::new(x_pixel2.round() as i32, y_pixel2.round() as i32);
    draw(point.swap_axis_if(steep), rfpart * x_gap);
    for i in 1..width.round() as i32 {
        draw(point.add_y(i).swap_axis_if(steep), 1.0);
    }
    draw(point.add_y_round(width).swap_axis_if(steep), fpart * x_gap);

    for x in x_pixel1 as i32 + 1..x_pixel2 as i32 {
        let fpart = inter_y.fract();
        let rfpart = 1.0 - fpart;

        let y = inter_y.floor() as i32;
        let point = Point::new(x, y);
        draw(point.swap_axis_if(steep), rfpart);
        for i in 1..width.round() as i32 {
            draw(point.add_y(i).swap_axis_if(steep), 1.0);
        }
        draw(point.add_y_round(width).swap_axis_if(steep), fpart);

        inter_y += gradient;
    }
}

pub fn line_downscale<F>(start: Point, end: Point, width: u32, mut draw: F)
where
    F: FnMut(Point, f32),
{
}

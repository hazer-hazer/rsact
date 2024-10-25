use core::mem::swap;

use crate::layout::size::PointExt;

use super::{color::Color, Rect};
use embedded_graphics::{
    prelude::{Dimensions, Point, Primitive},
    primitives::{PrimitiveStyle, PrimitiveStyleBuilder, Rectangle},
    Drawable, Pixel,
};
use num::{integer::Roots, pow::Pow, Zero};

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
// TODO: Canonize Line when constructing? Swap start and end in to always keep start.x < end.x?
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line {
    pub start: Point,
    pub end: Point,
}

impl Dimensions for Line {
    fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
        Rectangle::new(
            self.start,
            embedded_graphics::geometry::Size::new(
                (self.end.x - self.start.x).abs() as u32,
                (self.end.y - self.start.y).abs() as u32,
            ),
        )
    }
}

impl Primitive for Line {}

impl Line {
    pub fn new(start: Point, end: Point) -> Self {
        Self { start, end }
    }

    pub fn len_sq(&self) -> u32 {
        let dx = self.end.x - self.start.x;
        let dy = self.end.y - self.start.y;
        dx.pow(2) as u32 + dy.pow(2) as u32
    }

    // pub fn points(&self) -> impl Iterator<Item = Point> {
    //     for
    // }

    // pub fn distance_to(&self, point: Point) -> f32 {
    //     let dx = self.end.x - self.start.x;
    //     let dy = self.end.y - self.start.y;
    //     let len_sq = (dx.pow(2) + dy.pow(2)) as f32;

    //     if len_sq == 0.0 {
    //         ((point.x - self.start.x).pow(2) + (point.y - self.start.y).pow(2))
    //             as f32
    //     } else {
    //         // let t = ((point.x - self.start.x) * dx
    //         //     + (point.y - self.start.y) * dy) as f32
    //         //     / len_sq;
    //         // let t = t.clamp(0.0, 1.0);
    //         // let proj_x = self.start.x as f32 + t * dx as f32;
    //         // let proj_y = self.start.y as f32 + t * dy as f32;

    //         // ((point.x as f32 - proj_x).powi(2)
    //         //     + (point.y as f32 - proj_y).powi(2))
    //         // .sqrt()

    //         let t = ((point - self.start).dot(self.end - self.start) as f32
    //             / len_sq)
    //             .min(1.0)
    //             .max(0.0);
    //         let proj = self.start + (self.end - self.start).scale_round(t);
    //         point.dist_to(proj)
    //     }
    // }

    pub fn dist_to(&self, point: Point) -> f32 {
        // let l2 = self.start.dist_sq(self.end);
        let delta = self.end - self.start;
        let len_sq = (delta.x.pow(2) + delta.y.pow(2)) as f32;
        // Case when start == end
        // TODO: Can just be replaced with start == end check?
        if len_sq == 0.0 {
            point.dist_to(self.start)
        } else {
            // let t = ((point.x - self.start.x) * (self.end.x - self.start.x)
            //     + (point.y - self.start.y) * (self.end.y - self.start.y))
            //     as f32
            //     / l2;
            // let t =
            //     (point - self.start).dot(self.end - self.start) as f32 / len_sq;
            let t = (point.x - self.start.x) * delta.x
                + (point.y - self.start.y) * delta.y;
            let t = t as f32 / len_sq;
            let t = t.clamp(0.0, 1.0);
            let proj_x = self.start.x as f32 + t * delta.x as f32;
            let proj_y = self.start.y as f32 + t * delta.y as f32;

            ((point.x as f32 - proj_x).powi(2)
                + (point.y as f32 - proj_y).powi(2))
            .sqrt()
            // (point.x as f32 - )
            // point
            //     .dist_to(
            //         self.start
            //             .add_x_round(t * (self.end.x - self.start.x) as f32)
            //             .add_y_round(t * (self.end.y - self.start.y) as f32),
            //     )
            //     .sqrt()
        }
    }
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

// pub fn line_aa<F>(start: Point, end: Point, width: u32, mut draw: F)
// where
//     F: FnMut(Point, f32),
// {
//     let steep = (end.y - start.y).abs() > (end.x - start.x).abs();

//     let mut start = start.swap_axis_if(steep);
//     let mut end = end.swap_axis_if(steep);

//     if start.x > end.x {
//         core::mem::swap(&mut start.x, &mut end.x);
//         core::mem::swap(&mut start.y, &mut end.y);
//     }

//     let dx = end.x - start.x;
//     let dy = end.y - start.y;
//     let gradient = if dx > 0 { dy as f32 / dx as f32 } else { 1.0 };

//     let width = width as f32 * (1.0 + gradient * gradient).sqrt();

//     let len = ((dx * dx + dy * dy) as f32).sqrt();
//     let perp_x = -(dy as f32) / len;
//     let perp_y = (dx as f32) / len;

//     let end_x = start.x as f32;
//     let end_y = start.y as f32 - (width - 1.0) * 0.5
//         + gradient * (end_x - start.x as f32);
//     let x_gap = 1.0 - (start.x as f32 + 0.5 - end_x);
//     let x_pixel1 = end_x;
//     let y_pixel1 = end_y.floor();
//     let fpart = end_y.fract();
//     let rfpart = 1.0 - fpart;

//     // let step =

//     // Draw first endpoint
//     // let point = Point::new(x_pixel1.round() as i32, y_pixel1.round() as i32);
//     // draw(point.swap_axis_if(steep), rfpart * x_gap);
//     // for i in 1..width.round() as i32 {
//     //     draw(point.add_y(i).swap_axis_if(steep), 1.0);
//     // }
//     // draw(point.add_y_round(width).swap_axis_if(steep), fpart * x_gap);

//     let mut inter_y = end_y + gradient;

//     // Draw second endpoint
//     let end_x = end.x as f32;
//     let end_y =
//         // Note end_x was integer, end.x is integer too, so `gradient * (end_x - end.x)` is always 0
//         end.y as f32 - (width - 1.0) * 0.5 + gradient * (end_x - end.x as f32);
//     let x_gap = 1.0 - (end.x as f32 + 0.5 - end_x);
//     let x_pixel2 = end_x;
//     let y_pixel2 = end_y.floor();
//     let fpart = end_y.fract();
//     let rfpart = 1.0 - fpart;

//     // let point = Point::new(x_pixel2.round() as i32, y_pixel2.round() as i32);
//     // draw(point.swap_axis_if(steep), rfpart * x_gap);
//     // for i in 1..width.round() as i32 {
//     //     draw(point.add_y(i).swap_axis_if(steep), 1.0);
//     // }
//     // draw(point.add_y_round(width).swap_axis_if(steep), fpart * x_gap);

//     for x in x_pixel1 as i32 + 1..x_pixel2 as i32 {
//         let fpart = inter_y.fract();
//         let rfpart = 1.0 - fpart;

//         let y = inter_y.round() as i32;
//         let point = Point::new(x, y);
//         draw(point.swap_axis_if(steep), rfpart);
//         for i in 1..(width.round()) as i32 {
//             let shift_x = (perp_x * i as f32 * 0.5).round() as i32;
//             let shift_y = (perp_y * i as f32 * 0.5).round() as i32;
//             // draw(point.add_y(i).swap_axis_if(steep), 1.0);
//             draw(point.add_x(shift_x).add_y(shift_y).swap_axis_if(steep), 1.0);
//         }
//         draw(point.add_y_round(width).swap_axis_if(steep), fpart);

//         inter_y += gradient;
//     }
// }

// pub fn line_aa1<F>(start: Point, end: Point, width: u32, mut draw: F)
// where
//     F: FnMut(Point, f32),
// {
//     let dx = (end.x - start.x).abs();
//     let dy = (end.y - start.y).abs();

//     let mut x = start.x;
//     let mut y = start.y;

//     let x_step = if start.x < end.x { 1 } else { -1 };
//     let y_step = if start.y < end.y { 1 } else { -1 };

//     let mut err = if dx > dy { dx / 2 } else { -dy / 2 };
//     let radius = (width / 2) as i32;

//     let mut line_circle = |center: Point| {
//         for dy in -radius..=radius {
//             for dx in -radius..=radius {
//                 let dist = ((dx.pow(2) + dy.pow(2)) as f32).sqrt();
//                 let alpha = if dist <= radius as f32 {
//                     1.0
//                 } else {
//                     1.0 - (dist - radius as f32).clamp(0.0, 1.0)
//                 };
//                 draw(center.add_x(dx).add_y(dy), alpha);
//             }
//         }
//     };

//     if dx >= dy {
//         while x != end.x {
//             line_circle(Point::new(x, y));

//             err -= dy;
//             if err < 0 {
//                 y += y_step;
//                 err += dx;
//             }
//             x += x_step;
//         }
//     } else {
//         while y != end.y {
//             line_circle(Point::new(x, y));
//             err -= dx;
//             if err < 0 {
//                 x += x_step;
//                 err -= dy;
//             }
//             y += y_step;
//         }
//     }
// }

pub fn xiaolin_wu<F>(
    mut start: Point,
    mut end: Point,
    width: u32,
    mut draw_pixel: F,
) where
    F: FnMut(Point, f32),
{
    let steep = (end.y - start.y).abs() > (end.x - start.x).abs();

    start = start.swap_axis_if(steep);
    end = end.swap_axis_if(steep);

    if start.x > end.x {
        core::mem::swap(&mut start, &mut end);
        // core::mem::swap(&mut start.x, &mut end.x);
        // core::mem::swap(&mut start.y, &mut end.y);
    }

    let dx = end.x - start.x;
    if dx == 0 {
        for y in start.y..=end.y {
            for x in 0..width as i32 {
                draw_pixel(Point::new(start.x + x, y), 1.0);
            }
        }
        return;
    }

    let dy = end.y - start.y;
    if dy == 0 {
        for x in start.x..=end.x {
            for y in 0..width as i32 {
                draw_pixel(Point::new(x, start.y + y), 1.0);
            }
        }
        return;
    }

    let gradient = if dx == 0 { 1.0 } else { dy as f32 / dx as f32 };

    let x_end = start.x as f32;
    let y_end = start.y as f32 + gradient * (x_end - start.x as f32);
    // TODO: Optimize to 0.5
    // let x_gap = 1.0 - (start.x as f32 + 0.5).fract();
    let x_gap = 0.5;
    let x_pixel1 = x_end as i32;
    let y_pixel1 = y_end as i32;

    let point = Point::new(x_pixel1, y_pixel1);
    draw_pixel(point.swap_axis_if(steep), (1.0 - y_end.fract()) * x_gap);
    draw_pixel(point.add_y(1).swap_axis_if(steep), y_end.fract() * x_gap);

    let mut inter_y = y_end + gradient;

    let x_end = end.x as f32;
    let y_end = end.y as f32 + gradient * (x_end - end.x as f32);
    // let x_gap = (end.x as f32 + 0.5).fract();
    let x_gap = 0.5;
    let x_pixel2 = x_end as i32;
    let y_pixel2 = y_end as i32;

    let point = Point::new(x_pixel2, y_pixel2);

    draw_pixel(point.swap_axis_if(steep), (1.0 - y_end.fract()) * x_gap);
    draw_pixel(point.add_y(1).swap_axis_if(steep), y_end.fract() * x_gap);

    let half_w = width.div_ceil(2) as i32;
    for x in (x_pixel1 + 1)..x_pixel2 {
        let point = Point::new(x, inter_y as i32);
        draw_pixel(point.swap_axis_if(steep), 1.0 - inter_y.fract());
        for y in -half_w..half_w {
            draw_pixel(
                point.add_y(y).swap_axis_if(steep),
                1.0 - ((half_w.abs() - y.abs()) as f32 / half_w as f32)
                    * inter_y.fract(),
            );
        }
        draw_pixel(point.add_y(1).swap_axis_if(steep), inter_y.fract());
        inter_y += gradient;
    }
}

pub fn line_aa<F>(
    mut start: Point,
    mut end: Point,
    width: u32,
    mut draw_pixel: F,
) where
    F: FnMut(Point, f32),
{
    let steep = (end.y - start.y).abs() > (end.x - start.x).abs();

    start = start.swap_axis_if(steep);
    end = end.swap_axis_if(steep);

    if start.x > end.x {
        core::mem::swap(&mut start, &mut end);
    }

    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let gradient = if dx > 0 { dy as f32 / dx as f32 } else { 1.0 };

    let width = width as i32;
    let w = width as f32 * (1.0 + gradient.powi(2)).sqrt();
    let draw_width = w.round() as i32;

    let x_end = start.x as f32;
    let y_end = start.y as f32 - (w - 1.0) * 0.5
        + gradient * (x_end as f32 - start.x as f32);
    let x_gap = 1.0 - (start.x as f32 + 0.5 - x_end);
    let x_pixel1 = x_end;
    let y_pixel1 = y_end.floor();
    let fpart = y_end.fract();
    let rfpart = 1.0 - fpart;

    let point = Point::new(x_pixel1 as i32, y_pixel1 as i32);
    draw_pixel(point.swap_axis_if(steep), rfpart * x_gap);
    for w in 1..draw_width {
        draw_pixel(point.add_y(w).swap_axis_if(steep), 1.0);
    }
    draw_pixel(point.add_y(draw_width).swap_axis_if(steep), fpart * x_gap);

    let mut inter_y = y_end + gradient;

    let x_end = end.x as f32;
    let y_end =
        end.y as f32 - (w - 1.0) * 0.5 + gradient * (x_end - end.x as f32);
    let x_gap = 1.0 - (end.x as f32 + 0.5 - x_end);
    let x_pixel2 = x_end;
    let y_pixel2 = y_end.floor();
    let fpart = y_end.fract();
    let rfpart = 1.0 - fpart;

    let point = Point::new(x_pixel2 as i32, y_pixel2 as i32);
    draw_pixel(point.swap_axis_if(steep), rfpart * x_gap);
    for w in 1..draw_width {
        draw_pixel(point.add_y(w).swap_axis_if(steep), 1.0);
    }
    draw_pixel(point.add_y(draw_width).swap_axis_if(steep), fpart * x_gap);

    for x in x_pixel1.round() as i32 + 1..x_pixel2.round() as i32 {
        let fpart = inter_y.fract();
        let rfpart = 1.0 - fpart;
        let y = inter_y.floor() as i32;

        let point = Point::new(x, y);
        draw_pixel(point.swap_axis_if(steep), rfpart);
        for w in 1..draw_width {
            draw_pixel(point.add_y(w).swap_axis_if(steep), 1.0);
        }
        draw_pixel(point.add_y(draw_width).swap_axis_if(steep), fpart);
        inter_y += gradient;
    }
}

// Some strange non-working Gupta-Sproull
// pub fn line_aa<F>(
//     mut start: Point,
//     mut end: Point,
//     width: u32,
//     mut draw_pixel: F,
// ) where
//     F: FnMut(Point, f32),
// {
//     let dx = (end.x - start.x).abs();
//     let dy = (end.y - start.y).abs();

//     let mut x = start.x;
//     let mut y = start.y;

//     let x_step = if start.x < end.x { 1 } else { -1 };
//     let y_step = if start.y < end.y { 1 } else { -1 };

//     let mut d_error = 0;

//     if dx > dy {
//         let gradient = (dy << 16) / dx;

//         for _ in 0..=dx {
//             line_segment(Point::new(x, y), d_error, width, |point, blend| {
//                 draw_pixel(point, blend)
//             });
//             x += x_step;
//             d_error += gradient as i64;
//             if d_error >= (1 << 16) {
//                 y += y_step;
//                 d_error -= 1 << 16;
//             }
//         }
//     } else {
//         let gradient = (dx << 16) / dy;

//         for _ in 0..=dy {
//             line_segment(Point::new(x, y), d_error, width, |point, blend| {
//                 draw_pixel(point, blend)
//             });
//             y += y_step;
//             d_error += gradient as i64;
//             if d_error >= (1 << 16) {
//                 x += x_step;
//                 d_error -= 1 << 16;
//             }
//         }
//     }
// }

// fn line_segment<F>(point: Point, d_error: i64, width: u32, mut draw_pixel: F)
// where
//     F: FnMut(Point, f32),
// {
//     let err2 = d_error.pow(2);
//     let err = ((width as i64) << 16) - err2;

//     let blend = err as f32 / (width as f32 * (1 << 16) as f32);
//     draw_pixel(point, blend);
//     draw_pixel(point.add_y(1), blend * 0.7);
//     draw_pixel(point.add_y(-1), blend * 0.7);
// }

// pub fn line_aa<F>(
//     mut start: Point,
//     mut end: Point,
//     width: u32,
//     mut draw_pixel: F,
// ) where
//     F: FnMut(Point, f32),
// {
//     let dx = end.x - start.x;
//     let dy = end.y - start.y;

//     let adx = if dx < 0 { -dx } else { dx };
//     let ady = if dy < 0 { -dy } else { dy };
//     let mut x = start.x;
//     let mut y = start.y;

//     let step_x = if dx < 0 { -1 } else { 1 };
//     let step_y = if dy < 0 { -1 } else { 1 };

//     let (du, dv, mut u) =
//         if adx > ady { (adx, ady, end.x) } else { (ady, adx, end.y) };

//     let u_end = u + du;
//     let mut d = 2 * dv - du;
//     let incr_s = 2 * dv;
//     let incr_d = 2 * (dv - du);
//     let mut two_vdu = 0.0;
//     let inv_d = 1.0 / (2.0 * ((du.pow(2) + dv.pow(2)) as f32).sqrt());
//     let inv_d_2du = 2.0 * (du as f32 * inv_d);

//     if adx > ady {
//         loop {
//             let point = Point::new(x, y);
//             draw_pixel(point, two_vdu * inv_d);
//             draw_pixel(point.add_y(step_y), inv_d_2du - two_vdu * inv_d);
//             draw_pixel(point.add_y(-step_y), inv_d_2du + two_vdu * inv_d);

//             if d < 0 {
//                 two_vdu = (d + du) as f32;
//                 d += incr_s;
//             } else {
//                 two_vdu = (d - du) as f32;
//                 d += incr_d;
//                 y += step_y;
//             }
//             u += 1;
//             x += step_x;

//             if u < u_end {
//                 break;
//             }
//         }
//     } else {
//         loop {
//             let point = Point::new(x, y);
//             draw_pixel(point, two_vdu * inv_d);
//             draw_pixel(point.add_y(step_y), inv_d_2du - two_vdu * inv_d);
//             draw_pixel(point.add_y(-step_y), inv_d_2du + two_vdu * inv_d);

//             if d < 0 {
//                 two_vdu = (d + du) as f32;
//                 d += incr_s;
//             } else {
//                 two_vdu = (d - du) as f32;
//                 d += incr_d;
//                 x += step_x;
//             }
//             u += 1;
//             y += step_y;

//             if u < u_end {
//                 break;
//             }
//         }
//     }
// }

// pub fn line_aa<F>(
//     mut start: Point,
//     mut end: Point,
//     width: u32,
//     mut draw_pixel: F,
// ) where
//     F: FnMut(Point, f32),
// {
//     let mut draw_pixel = |point, blend: f32| {
//         draw_pixel(
//             point,
//             1.0014 + 0.0086 * blend - 1.4886 * blend.powi(2)
//                 + 0.5344 * blend.powi(3),
//         )
//     };

//     let steep = (end.y - start.y).abs() > (end.x - start.x).abs();

//     start = start.swap_axis_if(steep);
//     end = end.swap_axis_if(steep);

//     if start.x > end.x {
//         core::mem::swap(&mut start, &mut end);
//     }

//     let dx = end.x - start.x;
//     let dy = end.y - start.y;

//     let len = ((dx.pow(2) + dy.pow(2)) as f32).sqrt();
//     let sin = dy as f32 / len;
//     let cos = dx as f32 / len;

//     let mut dist = 0.0;
//     let mut d = 2 * dy - dx;
//     let mut y = start.y;
//     let half_w = width.div_ceil(2) as i32;

//     for x in start.x..=end.x {
//         let point = Point::new(x, y);
//         // draw_pixel(point.add_y(-1 - half_w).swap_axis_if(steep), dist + cos);
//         // for _w in -half_w..=half_w {
//         //     draw_pixel(point.add_y(half_w).swap_axis_if(steep), dist);
//         // }
//         // draw_pixel(point.add_y(1 + half_w).swap_axis_if(steep), dist - cos);

//         for w in -half_w - 1..=half_w + 1 {
//             draw_pixel(
//                 point.add_y(w).swap_axis_if(steep),
//                 dist - w as f32 * cos,
//             );
//         }

//         if d <= 0 {
//             dist += sin;
//             d += 2 * dy;
//         } else {
//             dist += sin - cos;
//             d += 2 * (dy - dx);
//             y += 1;
//         }
//     }
// }

use crate::geometry::*;
#[allow(unused)]
use num::Float as _;

// TODO: Canonize Line when constructing? Swap start and end in to always keep start.x < end.x?
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line {
    pub start: Point,
    pub end: Point,
}

impl Line {
    pub fn new(start: Point, end: Point) -> Self {
        Self { start, end }
    }

    pub fn translate(&self, by: Point) -> Self {
        let mut new = *self;
        new.start += by;
        new.end += by;
        new
    }

    pub fn translate_mut(&mut self, by: Point) -> &mut Self {
        self.start += by;
        self.end += by;
        self
    }

    pub fn with_angle(center: Point, angle: Angle, radius: f32) -> Self {
        let (sin, cos) = angle.to_radians().sin_cos();
        let end = center.add_x_round(cos * radius).add_y_round(sin * radius);
        Self::new(center, end)
    }

    pub fn len_sq(&self) -> u32 {
        let dx = self.end.x - self.start.x;
        let dy = self.end.y - self.start.y;
        dx.pow(2) as u32 + dy.pow(2) as u32
    }

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

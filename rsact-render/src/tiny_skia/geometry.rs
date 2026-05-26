use crate::{geometry::Rect, path::Path, tiny_skia::path::PathBuilderExt as _};

impl Into<tiny_skia::Rect> for Rect {
    fn into(self) -> tiny_skia::Rect {
        tiny_skia::Rect::from_xywh(
            self.top_left.x as f32,
            self.top_left.y as f32,
            self.size.width as f32,
            self.size.height as f32,
        )
        .unwrap()
    }
}

impl Into<tiny_skia::Path> for Path {
    fn into(self) -> tiny_skia::Path {
        // TODO: PathBuilder::with_capacity
        let mut builder = tiny_skia::PathBuilder::new();

        for segment in self.segments {
            match segment {
                crate::path::PathSegment::MoveTo(point) => {
                    builder.move_to(point.x as f32, point.y as f32);
                },
                crate::path::PathSegment::LineTo(point) => {
                    builder.line_to(point.x as f32, point.y as f32);
                },
                crate::path::PathSegment::ArcTo {
                    center,
                    radius,
                    start,
                    sweep,
                } => {
                    builder.arc(center, radius, start, sweep);
                },
                crate::path::PathSegment::Close => {
                    builder.close();
                },
            }
        }

        builder.finish().unwrap()
    }
}

use crate::{
    color::Color,
    geometry::{Rect, block_model::BlockModel, border::Border},
    renderer::{RenderResult, Renderer},
    style::{
        DrawStyle, StrokeAlignment,
        block::{BlockStyle, BorderStyle},
    },
};

#[derive(Clone, Copy)]
pub struct Block<C: Color> {
    pub border: Border<C>,
    pub rect: Rect,
    pub background: Option<C>,
}

impl<C: Color> Block<C> {
    /// Render this block using the renderer's primitive drawing methods.
    pub fn render<R: Renderer<Color = C>>(
        &self,
        renderer: &mut R,
    ) -> RenderResult {
        renderer.draw_rounded_rect(
            self.rect,
            self.border.radius.into_corner_radii(self.rect.size),
            DrawStyle {
                fill: self.background,
                stroke: self.border.color,
                stroke_width: self.border.width,
                stroke_alignment: StrokeAlignment::Inside,
            },
        )
    }

    // TODO: Find better way to construct Block. border width inside layout
    // makes it complex
    #[inline]
    pub fn from_layout_style(
        outer: Rect,
        BlockModel { border_width, padding: _ }: BlockModel,
        BlockStyle {
            background_color,
            border: BorderStyle { color: border_color, radius },
        }: BlockStyle<C>,
    ) -> Self {
        Self {
            border: Border {
                color: border_color.get(),
                width: border_width,
                radius,
            },
            rect: outer,
            background: background_color.get(),
        }
    }
}

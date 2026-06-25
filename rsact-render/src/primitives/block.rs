use crate::{
    color::Color,
    geometry::{CornerRadii, Rect, Size, block_model::BlockModel},
    renderer::{RenderResult, Renderer},
    style::{
        DrawStyle, StrokeAlignment,
        block::{BlockStyle, BorderStyle, OutlineStyle},
    },
};

/// High-level primitive for drawing arbitrary block

#[derive(Clone, Copy)]
pub struct Block<C: Color> {
    // TODO: Directly store [`DrawStyle`]?
    rect: Rect,
    background: Option<C>,

    // All properties from BorderStyle and OutlineStyle are resolved because
    // Block as other primitives are to be used by value without mutations
    // which allows caching them.
    border_color: Option<C>,
    border_radius: CornerRadii,
    border_width: u32,

    // Computed from outline_offset
    outline_size: Size,
    outline_color: Option<C>,
    outline_radius: CornerRadii,
    outline_width: u32,
}

impl<C: Color> Block<C> {
    /// Render this block using the renderer's primitive drawing methods.
    pub fn render<R: Renderer<Color = C>>(
        &self,
        renderer: &mut R,
    ) -> RenderResult {
        // TODO: Actually border_width = 0 can be used for hairline rendering.
        if self.border_color.is_some() || self.border_width > 0 {
            renderer.rounded_rect(
                self.rect,
                self.border_radius,
                &DrawStyle {
                    fill: self.background,
                    stroke: self.border_color,
                    stroke_width: self.border_width,
                    stroke_alignment: StrokeAlignment::Inside,
                },
            )?;
        }

        if self.outline_color.is_some() && self.outline_width > 0 {
            let outline_rect = self.rect.resized_center(self.outline_size);
            renderer.rounded_rect(
                outline_rect,
                self.outline_radius,
                &DrawStyle {
                    fill: None,
                    stroke: self.outline_color,
                    stroke_width: self.outline_width,
                    stroke_alignment: StrokeAlignment::Outside,
                },
            )?;
        }

        Ok(())
    }

    #[inline]
    pub fn from_layout_style(
        rect: Rect,
        BlockModel { border_width, padding: _ }: BlockModel,
        BlockStyle {
            background_color,
            border: BorderStyle { color: border_color, radius },
            outline:
                OutlineStyle {
                    color: outline_color,
                    radius: outline_radius,
                    offset: outline_offset,
                    width: outline_width,
                },
        }: BlockStyle<C>,
    ) -> Self {
        let outline_size = rect.size + Size::new_equal(outline_offset * 2);

        Self {
            rect,
            background: background_color.get(),

            border_color: border_color.get(),
            border_radius: radius.into_corner_radii(rect.size),
            border_width,

            outline_size,
            outline_color: outline_color.get(),
            outline_radius: outline_radius.into_corner_radii(outline_size),
            outline_width,
        }
    }
}

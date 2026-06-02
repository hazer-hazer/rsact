use crate::font::FontCtx;
use crate::render::prelude::*;
use crate::widget::ctx::WidgetCtx;
use crate::{
    layout::DevHoveredLayout,
    prelude::{BlockStyle, BorderStyle},
    render::color::Color,
};
use log::debug;
use rsact_reactive::prelude::*;

#[derive(Default)]
pub struct DevTools {
    pub enabled: bool,
    pub hovered: Option<DevHoveredEl>,
}

pub struct DevHoveredEl {
    pub layout: DevHoveredLayout,
}

impl DevHoveredEl {
    fn model<C: Color>(area: Rect, color: C) -> Block<C> {
        Block {
            border: Border::new(
                BlockStyle::base().border(BorderStyle::base().color(color)),
                BlockModel::zero().border_width(1),
            ),
            rect: area,
            background: None,
        }
    }

    pub fn draw<W: WidgetCtx>(
        &self,
        r: &mut W::Renderer,
        font_ctx: Signal<FontCtx, ReadOnly>,
        // TODO: Render on bottom right corner of the viewport.
        viewport: Size,
    ) -> RenderResult {
        let area = self.layout.area;

        let [text_color, inner_color, padding_color, ..] = W::Color::accents();

        Self::model(area, padding_color).render(r)?;
        if let Some(padding) = self.layout.padding() {
            Self::model(area - padding, inner_color).render(r)?;
        }

        // TODO: Viewport-dependent font props resolution similar to layout computation for text widget.
        font_ctx.with(|font_ctx| {
            font_ctx.render::<W>(
                crate::font::Font::Auto,
                &format!("{}", self.layout),
                crate::font::ResolvedFontProps {
                    size: 12,
                    style: crate::font::FontStyle::Normal,
                },
                area,
                text_color,
                r,
            )
        })
    }
}

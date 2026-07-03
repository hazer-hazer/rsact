use crate::{
    el::ctx::WidgetCtx,
    font::FontCtx,
    layout::DevHoveredLayout,
    prelude::BlockStyle,
    render::{color::Color, prelude::*},
};
use rsact_reactive::prelude::*;

#[derive(Default)]
pub struct DevTools {
    pub enabled: bool,
    pub hovered: Option<DevHoveredEl>,
}

pub struct DevHoveredEl {
    pub layout: DevHoveredLayout,
}

impl PartialEq for DevHoveredEl {
    fn eq(&self, other: &Self) -> bool {
        // TODO: Better equality
        self.layout.area == other.layout.area
    }
}

impl DevHoveredEl {
    fn block<C: Color>(rect: Rect, color: C) -> Block<C> {
        Block::from_layout_style(
            rect,
            BlockModel::zero(),
            BlockStyle::base()
                .outline(OutlineStyle::base().width(1).color(color)),
        )
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

        Self::block(area, padding_color).render(r)?;
        if let Some(padding) = self.layout.padding() {
            Self::block(area - padding, inner_color).render(r)?;
        }

        // TODO: Viewport-dependent font props resolution similar to layout
        // computation for text widget.
        font_ctx.with(|font_ctx| {
            font_ctx.render::<W>(
                crate::font::Font::Auto,
                &format!("{}", self.layout),
                crate::font::ResolvedFontProps {
                    size: 12,
                    style: crate::font::FontStyle::Normal,
                },
                Rect::top_left(viewport),
                text_color,
                r,
            )
        })
    }
}

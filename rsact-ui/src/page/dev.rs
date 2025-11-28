use embedded_graphics::{
    mono_font::{MonoTextStyleBuilder, ascii::FONT_8X13},
    prelude::Point,
    primitives::Rectangle,
};
use embedded_text::{TextBox, style::TextBoxStyleBuilder};

use crate::{
    layout::DevHoveredLayout,
    prelude::{BlockModel, BlockStyle, BorderStyle, Size},
    render::{Block, Border, Renderable, Renderer, color::Color},
    widget::RenderResult,
};

#[derive(Clone, Default)]
pub struct DevTools {
    pub enabled: bool,
    pub hovered: Option<DevHoveredEl>,
}

#[derive(Clone)]
pub struct DevHoveredEl {
    pub layout: DevHoveredLayout,
}

impl DevHoveredEl {
    fn model<C: Color>(area: Rectangle, color: C) -> Block<C> {
        Block {
            border: Border::new(
                BlockStyle::base().border(BorderStyle::base().color(color)),
                BlockModel::zero().border_width(1),
            ),
            rect: area,
            background: None,
        }
    }

    pub fn draw<C: Color>(
        &self,
        r: &mut impl Renderer<Color = C>,
        viewport: Size,
    ) -> RenderResult {
        let area = self.layout.area;

        let [text_color, inner_color, padding_color, ..] = C::accents();

        Self::model(area, padding_color).render(r)?;
        if let Some(padding) = self.layout.padding() {
            Self::model(area - padding, inner_color).render(r)?;
        }

        // Ignore error, TextBox sometimes fails
        TextBox::with_textbox_style(
            &format!("{}", self.layout),
            Rectangle::new(Point::zero(), viewport.into()),
            MonoTextStyleBuilder::new()
                .font(&FONT_8X13)
                .text_color(text_color)
                .background_color(C::default_background())
                .build(),
            TextBoxStyleBuilder::new()
                .alignment(embedded_text::alignment::HorizontalAlignment::Right)
                .vertical_alignment(
                    embedded_text::alignment::VerticalAlignment::Bottom,
                )
                .build(),
        )
        .render(r)?;

        Ok(())
    }
}

use embedded_graphics::{
    mono_font::{ascii::FONT_8X13, MonoTextStyleBuilder},
    prelude::Point,
    primitives::Rectangle,
};
use embedded_text::{style::TextBoxStyleBuilder, TextBox};

use crate::{
    layout::DevHoveredLayout,
    prelude::{BlockModel, BlockStyle, BorderStyle, Size},
    render::{color::Color, Block, Border, Renderer},
    widget::DrawResult,
};

#[derive(Clone, Copy)]
pub struct DevTools {
    pub enabled: bool,
    pub hovered: Option<DevHoveredEl>,
}

#[derive(Clone, Copy)]
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
    ) -> DrawResult {
        let area = self.layout.area;

        let [text_color, inner_color, padding_color, ..] = C::accents();

        r.block(Self::model(area, padding_color))?;
        if let Some(padding) = self.layout.layout.kind.padding() {
            r.block(Self::model(area - padding, inner_color))?;
        }

        let area_text = format!(
            "{} {}x{}({}){}",
            self.layout.layout.kind,
            area.size.width,
            area.size.height,
            self.layout.size,
            if self.layout.children_count > 0 {
                format!(" [{}]", self.layout.children_count)
            } else {
                alloc::string::String::new()
            },
        );

        // Ignore error, TextBox sometimes fails
        r.mono_text(TextBox::with_textbox_style(
            &area_text,
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
        ))
        .ok();

        Ok(())
    }
}
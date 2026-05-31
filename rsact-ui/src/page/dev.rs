use crate::render::prelude::*;
use crate::{
    layout::DevHoveredLayout,
    prelude::{BlockStyle, BorderStyle},
    render::color::Color,
};

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

    pub fn draw<C: Color, R>(&self, _r: &mut R, _viewport: Size) -> RenderResult
    where
        R: Renderer<Color = C>,
    {
        todo!()
        // use embedded_graphics::{
        //     Drawable as _,
        //     mono_font::{MonoTextStyleBuilder, ascii::FONT_8X13},
        //     prelude::Point,
        // };
        // use embedded_text::{TextBox, style::TextBoxStyleBuilder};

        // let area = self.layout.area;

        // let [text_color, inner_color, padding_color, ..] = C::accents();

        // Self::model(area, padding_color).render(r)?;
        // if let Some(padding) = self.layout.padding() {
        //     Self::model(area - padding, inner_color).render(r)?;
        // }

        // // Ignore error, TextBox sometimes fails
        // let eg_viewport = embedded_graphics::primitives::Rectangle::new(
        //     Point::zero(),
        //     viewport.into(),
        // );
        // let _ = TextBox::with_textbox_style(
        //     &format!("{}", self.layout),
        //     eg_viewport,
        //     MonoTextStyleBuilder::new()
        //         .font(&FONT_8X13)
        //         .text_color(text_color)
        //         .background_color(C::default_background())
        //         .build(),
        //     TextBoxStyleBuilder::new()
        //         .alignment(embedded_text::alignment::HorizontalAlignment::Right)
        //         .vertical_alignment(
        //             embedded_text::alignment::VerticalAlignment::Bottom,
        //         )
        //         .build(),
        // )
        // .draw(r)
        // .map_err(|_| ());

        // Ok(())
    }
}

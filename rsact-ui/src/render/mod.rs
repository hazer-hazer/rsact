use crate::{
    layout::{block_model::BlockModel, padding::Padding, size::Size},
    style::block::{BlockStyle, BorderRadius, BorderStyle},
    widget::DrawResult,
};
use alpha::{AlphaDrawTarget, AlphaDrawable};
use color::Color;
use embedded_graphics::{
    prelude::DrawTarget,
    primitives::{
        PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, RoundedRectangle,
        StyledDrawable,
    },
    Drawable, Pixel,
};

pub mod alpha;
pub mod color;
pub mod draw_target;
pub mod primitives;

#[derive(Debug, Clone, Copy)]
pub struct Border<C: Color>
where
    C: Copy,
{
    pub color: Option<C>,
    pub width: u32,
    pub radius: BorderRadius,
}

impl<C: Color> Border<C> {
    // pub fn new() -> Self {
    //     Self { color: None, width: 1, radius: BorderRadius::zero() }
    // }

    pub fn new(block_style: BlockStyle<C>, block_model: BlockModel) -> Self {
        Self {
            color: block_style.border.color.get(),
            width: block_model.border_width,
            radius: block_style.border.radius,
        }
    }

    pub fn zero() -> Self {
        Self { color: None, width: 0, radius: 0.into() }
    }

    pub fn color(mut self, color: Option<C>) -> Self {
        self.color = color;
        self
    }

    pub fn width(mut self, width: u32) -> Self {
        self.width = width;
        self
    }

    pub fn radius(mut self, radius: impl Into<BorderRadius>) -> Self {
        self.radius = radius.into();
        self
    }

    // Make Block for border used as outline. Background color is always removed
    // to avoid drawing above element.
    pub fn into_outline(self, bounds: Rectangle) -> Block<C> {
        Block { rect: bounds, background: None, border: self }
    }

    pub fn into_block(
        self,
        bounds: Rectangle,
        background: Option<C>,
    ) -> Block<C> {
        Block { rect: bounds, background, border: self }
    }
}

impl<C: Color> Into<Padding> for Border<C> {
    fn into(self) -> Padding {
        self.width.into()
    }
}

#[derive(Clone, Copy)]
pub struct Block<C: Color> {
    pub border: Border<C>,
    pub rect: Rectangle,
    pub background: Option<C>,
}

impl<C: Color> Block<C> {
    fn style(&self) -> PrimitiveStyle<C> {
        let style = PrimitiveStyleBuilder::new()
            .stroke_width(self.border.width)
            .stroke_alignment(
                embedded_graphics::primitives::StrokeAlignment::Inside,
            );

        let style = if let Some(border_color) = self.border.color {
            style.stroke_color(border_color)
        } else {
            style
        };

        let style = if let Some(background) = self.background {
            style.fill_color(background)
        } else {
            style
        };

        style.build()
    }
}

impl<C: Color> Drawable for Block<C> {
    type Color = C;
    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        RoundedRectangle::new(
            self.rect,
            self.border.radius.into_corner_radii(self.rect.size),
        )
        .draw_styled(&self.style(), target)
        .ok()
        .unwrap();

        Ok(())
    }
}

impl<C: Color> AlphaDrawable for Block<C> {
    type Color = C;

    fn draw_alpha<A>(&self, target: &mut A) -> DrawResult
    where
        A: AlphaDrawTarget<Color = Self::Color>,
    {
        // TODO: RoundedRectangle AA
        self.draw(target).ok().unwrap();
        Ok(())
    }
}

impl<C: Color + Copy> Block<C> {
    // pub fn new_filled(bounds: Rectangle, background: Option<C>) -> Self {
    //     Self { border: Border::zero(), rect: bounds, background }
    // }

    // TODO: Find better way to construct Block. border width inside layout
    // makes it complex
    #[inline]
    pub fn from_layout_style(
        outer: Rectangle,
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

/// Trait to pass any Drawable + AlphaDrawable to Renderer using `render` call instead of `Renderer::render`
pub trait Renderable<C: Color>:
    Sized + Drawable<Color = C> + AlphaDrawable<Color = C>
{
    fn render(&self, renderer: &mut impl Renderer<Color = C>) -> DrawResult {
        renderer.render(self)
    }
}

impl<C: Color, T> Renderable<C> for T where
    T: Drawable<Color = C> + AlphaDrawable<Color = C> + Sized
{
}

// pub trait Renderable {
//     type Color: Color;

//     fn render(
//         &self,
//         target: &mut impl DrawTarget<Color = Self::Color>,
//     ) -> DrawResult;

//     fn render_aa(
//         &self,
//         target: &mut impl AlphaDrawTarget<Color = Self::Color>,
//     ) -> DrawResult {
//         self.render(target)
//     }
// }

// // TODO: Get rid of embedded_graphics usage?
// impl<C: Color, T> Renderable for T
// where
//     T: Drawable<Color = C> + AlphaDrawable<Color = C>,
// {
//     type Color = C;

//     fn render(
//         &self,
//         target: &mut impl DrawTarget<Color = Self::Color>,
//     ) -> DrawResult {
//         self.draw(target).ok().unwrap();
//         Ok(())
//     }

//     fn render_aa(
//         &self,
//         target: &mut impl AlphaDrawTarget<Color = Self::Color>,
//     ) -> DrawResult {
//         self.draw_alpha(target).unwrap();
//         Ok(())
//     }
// }

// TODO: Custom MonoText struct with String to pass from Canvas widget. Lifetime
// in TextBox require Canvas only to draw 'static strings

pub type Alpha = f32;

pub trait Renderer {
    type Color: Color;
    type Options: PartialEq + Default;

    fn new(viewport: Size) -> Self;
    fn set_options(&mut self, options: Self::Options);

    // TODO: Generic targets
    // TODO: This is the same as implementing Drawable for Renderer
    fn finish_frame(&self, target: &mut impl DrawTarget<Color = Self::Color>);

    fn clear(&mut self, color: Self::Color) -> DrawResult;
    fn clipped(
        &mut self,
        area: Rectangle,
        f: impl FnOnce(&mut Self) -> DrawResult,
    ) -> DrawResult;
    fn on_layer(
        &mut self,
        index: usize,
        f: impl FnOnce(&mut Self) -> DrawResult,
    ) -> DrawResult;

    fn pixel_iter(
        &mut self,
        mut pixels: impl Iterator<Item = Pixel<Self::Color>>,
    ) -> DrawResult {
        pixels.try_for_each(|pixel| self.pixel(pixel))
    }

    // fn pixel_iter_alpha(
    //     &mut self,
    //     pixels: impl Iterator<Item = (Pixel<Self::Color>, Alpha)>,
    // ) -> DrawResult;

    fn render(
        &mut self,
        renderable: &impl Renderable<Self::Color>,
    ) -> DrawResult;

    fn translucent_pixel_iter(
        &mut self,
        mut pixels: impl Iterator<Item = Option<Pixel<Self::Color>>>,
    ) -> DrawResult {
        pixels.try_for_each(|pixel| {
            if let Some(pixel) = pixel {
                self.pixel(pixel)
            } else {
                Ok(())
            }
        })
    }

    fn pixel(&mut self, pixel: Pixel<Self::Color>) -> DrawResult;
    // fn pixel_alpha(
    //     &mut self,
    //     pixel: Pixel<Self::Color>,
    //     alpha: Alpha,
    // ) -> DrawResult;
}

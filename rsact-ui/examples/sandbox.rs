use embedded_canvas::CanvasAt;
use embedded_graphics::{
    pixelcolor::{raw::ToBytes, Rgb888},
    prelude::{Dimensions, DrawTarget, Point, PointsIter, RgbColor as _},
    Drawable, Pixel,
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use num::integer::Roots;
use rsact_reactive::prelude::*;
use rsact_ui::{
    event::{message::Message, simulator::simulator_single_encoder},
    layout::size::Size,
    render::color::RgbColor,
    style::accent::AccentStyler,
    ui::UI,
    widget::{
        bar::Bar, button::Button, flex::Flex, knob::Knob, mono_text::MonoText,
        SizedWidget, Widget as _,
    },
};
use std::{
    convert::Infallible,
    time::{Duration, Instant},
};

const WINDOW: [Point; 9] = [
    Point::new(-1, -1),
    Point::new(0, -1),
    Point::new(1, -1),
    Point::new(-1, 0),
    Point::new(0, 0),
    Point::new(1, 0),
    Point::new(-1, 1),
    Point::new(0, 1),
    Point::new(1, 1),
];

const GAUSSIAN: [f32; 9] = [
    1. / 16.,
    1. / 8.,
    1. / 16.,
    1. / 8.,
    1. / 4.,
    1. / 8.,
    1. / 16.,
    1. / 8.,
    1. / 16.,
];

struct Smoother<C: RgbColor> {
    canvas: CanvasAt<C>,
}

impl<C: RgbColor> Smoother<C> {
    fn new(size: Size) -> Self {
        Self { canvas: CanvasAt::new(Point::zero(), size.into()) }
    }

    fn window(&self, point: Point) -> impl Iterator<Item = Option<C>> + '_ {
        WINDOW.iter().map(move |nb| self.canvas.get_pixel(point + *nb))
    }
}

impl<C: RgbColor> Drawable for Smoother<C> {
    type Color = C;
    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        const WINDOW_SIZE: u16 = WINDOW.len() as u16;
        target.draw_iter(self.bounding_box().points().filter_map(|point| {
            self.canvas.get_pixel(point).map(|pixel| {
                let mut nbs = self.window(point).collect::<Vec<_>>();

                // let mean = nbs.iter().copied().fold(pixel, |mean, color| {
                //     color.map(|color| mean.mix(0.5, color)).unwrap_or(mean)
                // });

                // let rms = nbs.iter().copied().fold(pixel, |rms, color| {
                //     color
                //         .map(|color| {
                //             rms.fold(color, |rms, color| {
                //                 ((rms as u32).pow(2) + (color as u32).pow(2))
                //                     .sqrt()
                //                     as u8
                //             })
                //         })
                //         .unwrap_or(pixel)
                // });

                let gaussian = nbs.iter().copied().enumerate().fold(
                    pixel,
                    |gau, (index, color)| {
                        color
                            .map(|color| gau.mix(GAUSSIAN[index], color))
                            .unwrap_or(pixel)
                    },
                );

                Pixel(point, gaussian)
            })
        }))
    }
}

impl<C: RgbColor> Dimensions for Smoother<C> {
    fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
        self.canvas.bounding_box()
    }
}

impl<C: RgbColor> DrawTarget for Smoother<C> {
    type Color = C;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        self.canvas.draw_iter(pixels)
    }
}

fn main() {
    let output_settings = OutputSettingsBuilder::new().scale(1).build();

    let mut window = Window::new("SANDBOX", &output_settings);

    let mut display =
        SimulatorDisplay::<Rgb888>::new(Size::new(480, 270).into());

    window.update(&display);

    // let page1 = Flex::row(vec![Button::new("kek")
    //     .on_click(|| {
    //         println!("Go to page 2");
    //         Some(Message::GoTo(2))
    //     })
    //     .el()])
    // .fill()
    // .el();

    // let page2 = Flex::row(vec![Button::new("lol")
    //     .on_click(|| {
    //         println!("Go to page 1");
    //         Some(Message::GoTo(1))
    //     })
    //     .el()])
    // .fill()
    // .el();

    let page = Flex::row(vec![
        Bar::horizontal(127u8).el(),
        Knob::new(127u8).size(50).el(),
    ])
    .fill();

    let mut ui = UI::single_page(
        page,
        display.bounding_box().size,
        AccentStyler::new(Rgb888::RED),
    )
    .on_exit(|| std::process::exit(0));

    ui.current_page().auto_focus();
    // ui.page(2).auto_focus();

    let mut smoother = Smoother::new(display.bounding_box().size.into());

    let mut fps = 0;
    let mut last_time = Instant::now();
    loop {
        let now = Instant::now();
        if now - last_time >= Duration::from_secs(1) {
            println!("{fps}FPS");
            fps = 0;
            last_time = now;
        } else {
            fps += 1;
        }

        ui.tick(
            window
                .events()
                .map(simulator_single_encoder)
                .filter_map(|e| e)
                .inspect(|e| println!("Event: {e:?}")),
        );
        ui.draw(&mut smoother).unwrap();
        smoother.draw(&mut display).unwrap();

        window.update(&display);
    }
}

use embedded_graphics::{
    pixelcolor::{BinaryColor, Rgb888},
    prelude::{Dimensions, RgbColor},
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rand::{Rng, random, rng};
use rsact_reactive::prelude::*;
use rsact_ui::{event::simulator::simulator_single_encoder, prelude::*};
use u8g2_fonts::FontRenderer;

fn main() {
    let output_settings =
        OutputSettingsBuilder::new().max_fps(10000).scale(5).build();

    let mut window = Window::new("FLEX", &output_settings);

    let mut display =
        SimulatorDisplay::<BinaryColor>::new(Size::new(128, 80).into());

    window.update(&display);

    let page = Scrollable::vertical(
        col![
            Button::new("Abcdefghijklmnopqr"),
            Button::new("Abcdefghijklmnopqr"),
            Button::new("Abcdefghijklmnopqr"),
            Button::new("Abcdefghijklmnopqr"),
            Button::new("Abcdefghijklmnopqr"),
            Button::new("Abcdefghijklmnopqr"),
            Button::new("Abcdefghijklmnopqr")
        ]
        .gap(5u32)
        .fill_width(),
    )
    .fill()
    .el();

    let mut ui = UI::new_with_buffer_renderer(
        display.bounding_box().size.inert(),
        NullStyler,
        BinaryColor::Off,
    )
    .auto_focus()
    .on_exit(|| std::process::exit(0))
    .with_page(SinglePage, page);

    loop {
        ui.tick(
            window
                .events()
                .filter_map(simulator_single_encoder)
                .inspect(|e| println!("Event: {e:?}")),
        );
        ui.render(&mut display);

        window.update(&display);
    }
}

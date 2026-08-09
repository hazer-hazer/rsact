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
    env_logger::init();

    let output_settings = OutputSettingsBuilder::new().scale(5).build();

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

    let viewport: Size = display.bounding_box().size.into();
    // The framebuffer is the APPLICATION's — rsact never allocates one
    // and never owns one. On a device this would be a `StaticCell` array
    // placed wherever that board wants it (SDRAM, DTCM, a DMA pool).
    let mut renderer = EGRenderer::<Rgb888, AntiAliasingDisabled, _>::new(
        viewport,
        vec![0u32; viewport.area() as usize].into_boxed_slice(),
    );

    let mut ui = UI::new(Theme::default(), viewport)
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
        if ui.render(&mut renderer) {
            // The transport is the application's too: rsact says WHAT changed,
            // the app decides how it reaches the panel.
            ui.with_damage(|d| renderer.output_regions(&mut display, d));
        }

        window.update(&display);
    }
}

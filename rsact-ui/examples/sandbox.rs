use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{Dimensions, RgbColor},
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rsact_ui::{
    col,
    event::simulator::simulator_single_encoder,
    page::id::SinglePage,
    prelude::{IntoInert, Select, create_signal, *},
    row,
    style::theme::Theme,
    ui::UI,
    widget::{SizedWidget, Widget, flex::Flex},
};
use std::time::{Duration, Instant};

fn main() {
    env_logger::init();

    let output_settings = OutputSettingsBuilder::new().scale(3).build();

    let mut window = Window::new("SANDBOX", &output_settings);

    let mut display =
        SimulatorDisplay::<Rgb888>::new(Size::new(480, 270).into());

    window.update(&display);

    let selected = create_signal(0);
    let select = Select::vertical(selected, vec![0, 1, 2, 3].inert());

    let page = row![col![select]].center().fill();

    let viewport: Size = display.bounding_box().size.into();
    // The framebuffer is the APPLICATION's — rsact never allocates one
    // and never owns one. On a device this would be a `StaticCell` array
    // placed wherever that board wants it (SDRAM, DTCM, a DMA pool).
    let mut renderer = EGRenderer::<Rgb888, AntiAliasingDisabled, _>::new(
        viewport,
        vec![0u32; viewport.area() as usize].into_boxed_slice(),
    );

    let mut ui = UI::new(Theme::default(), viewport)
        .with_page(SinglePage, page.el())
        .on_exit(|| std::process::exit(0));

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

        if ui.render(&mut renderer) {
            // The transport is the application's too: rsact says WHAT changed,
            // the app decides how it reaches the panel.
            ui.with_damage(|d| renderer.output_regions(&mut display, d));
        }
        window.update(&display);
    }
}

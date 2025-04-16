use embedded_graphics::{
    pixelcolor::{Rgb565, Rgb888},
    prelude::{Dimensions, RgbColor},
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rsact_ui::{
    col,
    event::simulator::simulator_single_encoder,
    layout::size::Size,
    page::id::SinglePage,
    prelude::{IntoInert, Select, Slider, create_signal},
    render::{AntiAliasing, RendererOptions},
    row,
    style::{NullStyler, accent::AccentStyler},
    ui::UI,
    widget::{SizedWidget, Widget, flex::Flex},
};
use std::time::{Duration, Instant};

fn main() {
    let output_settings =
        OutputSettingsBuilder::new().max_fps(10000).scale(3).build();

    let mut window = Window::new("SANDBOX", &output_settings);

    let mut display =
        SimulatorDisplay::<Rgb565>::new(Size::new(160, 80).into());

    window.update(&display);

    // let wt_depth = cresi

    let page = row![col![
        // Select
    ]]
    .center()
    .fill();

    let mut ui = UI::new_with_buffer_renderer(
        display.bounding_box().size.inert(),
        NullStyler,
        Rgb565::WHITE,
    )
    .with_page(SinglePage, page.el())
    .with_renderer_options(
        RendererOptions::new().anti_aliasing(AntiAliasing::Enabled),
    )
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

        ui.draw(&mut display);
        window.update(&display);
    }
}

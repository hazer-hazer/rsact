use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{Dimensions, RgbColor},
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rsact_ui::{
    el::El,
    event::simulator::simulator_single_encoder,
    layout::size::Size,
    page::id::SinglePage,
    prelude::{Button, IntoInert},
    render::draw_target::{AntiAliasing, LayeringRendererOptions},
    style::accent::AccentStyler,
    ui::UI,
    widget::{flex::Flex, SizedWidget, Widget, WidgetCtx},
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn main() {
    let output_settings =
        OutputSettingsBuilder::new().max_fps(10000).scale(3).build();

    let mut window = Window::new("SANDBOX", &output_settings);

    let mut display =
        SimulatorDisplay::<Rgb888>::new(Size::new(480, 270).into());

    window.update(&display);

    let page = Flex::row([]).fill();

    let mut ui = UI::new(
        display.bounding_box().size.inert(),
        AccentStyler::new(Rgb888::RED),
    )
    .with_page(SinglePage, page.el())
    .with_renderer_options(
        LayeringRendererOptions::new().anti_aliasing(AntiAliasing::Enabled),
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

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
    layout::size::Size,
    page::id::SinglePage,
    prelude::{create_signal, IntoInert, Select, SignalMap},
    render::{AntiAliasing, RendererOptions},
    row,
    style::accent::AccentStyler,
    ui::UI,
    widget::{flex::Flex, SizedWidget, Widget},
};
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq, PartialOrd)]
enum WidgetTab {
    Button,
}

fn main() {
    let output_settings = OutputSettingsBuilder::new().max_fps(10000).build();

    let mut window = Window::new("Widget gallery", &output_settings);

    let mut display =
        SimulatorDisplay::<Rgb888>::new(Size::new(640, 360).into());

    window.update(&display);

    let widget = create_signal(WidgetTab::Button);
    let select_widget = Select::vertical(widget, vec![].inert());

    let widget_view = widget.map(|widget| {

    });

    let page = row![col![select_widget, ]].center().fill();

    let mut ui = UI::new_with_buffer_renderer(
        display.bounding_box().size.inert(),
        AccentStyler::new(Rgb888::RED),
        Rgb888::WHITE,
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

        ui.render(&mut display);
        window.update(&display);
    }
}

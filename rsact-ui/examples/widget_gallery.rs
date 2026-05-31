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
    prelude::*,
    prelude::{IntoInert, Select, SignalMap, create_signal},
    row,
    style::theme::Theme,
    ui::UI,
    widget::{SizedWidget, Widget, container::Container, flex::Flex},
};
use std::{
    fmt::Display,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
enum WidgetTab {
    Button,
}

impl WidgetTab {
    fn each() -> impl Iterator<Item = Self> {
        [Self::Button].into_iter()
    }
}

impl Display for WidgetTab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WidgetTab::Button => write!(f, "Button"),
        }
    }
}

fn main() {
    env_logger::init();

    let output_settings = OutputSettingsBuilder::new().build();

    let mut window = Window::new("Widget gallery", &output_settings);

    let mut display =
        SimulatorDisplay::<Rgb888>::new(Size::new(640, 360).into());

    window.update(&display);

    let widget = create_signal(WidgetTab::Button);
    let select_widget =
        Select::vertical(widget, WidgetTab::each().collect::<Vec<_>>().inert());

    let widget_view = Container::new(
        dynamic(move || match widget.get() {
            WidgetTab::Button => Button::new("Button").el(),
        })
        .el(),
    );

    let page = row![col![select_widget].fill(), col![widget_view].fill()]
        .center()
        .fill();

    let mut ui = UI::new(
        Theme::default(),
        TinySkiaRenderer::new(display.bounding_box().size.into()),
    )
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

        ui.render(&mut display);
        window.update(&display);
    }
}

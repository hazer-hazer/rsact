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
    prelude::{IntoInert, Select, SignalMap, create_signal, *},
    row,
    style::theme::Theme,
    ui::UI,
    widget::{
        SizedWidget, Widget, canvas::Canvas, checkbox::Checkbox,
        container::Container, flex::Flex,
    },
};
use std::{
    fmt::Display,
    time::{Duration, Instant},
};

type Color = tiny_skia::Color;
type W = Wtf<TinySkiaRenderer<Color>, SinglePage, Theme<Color>, ()>;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
enum WidgetTab {
    Container,
    Button,
    Canvas,
    Checkbox,
    Label,
}

impl WidgetTab {
    fn each() -> impl Iterator<Item = Self> {
        [
            Self::Container,
            Self::Button,
            Self::Checkbox,
            Self::Label,
            Self::Canvas,
        ]
        .into_iter()
    }
}

impl Display for WidgetTab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WidgetTab::Container => write!(f, "Container"),
            WidgetTab::Button => write!(f, "Button"),
            WidgetTab::Canvas => write!(f, "Canvas"),
            WidgetTab::Checkbox => write!(f, "Checkbox"),
            WidgetTab::Label => write!(f, "Label"),
        }
    }
}

fn container() -> impl View<W> {
    row![
        "Container is a widget with a single child. You can set padding, border and its radius, background color, and alignment of the child.",

        col![
            "Padding [top 5px, right 10, bottom 15, left 20]",
            Container::new(Edge::new().size(Size::new_equal(50)).style(|base, _| {
                base.background_color(tiny_skia::Color::from_rgba8(255, 128, 0, 255))
            }))
            .padding(Padding::new(5, 10, 15, 20))
        ],

        col![
            "Border [width 5px, color red, radius 10px]",
            Container::new(
                Edge::new().size(Size::new_equal(50)).border_width(5).style(
                    |base, _| {
                        base.background_color(tiny_skia::Color::from_rgba8(
                            0, 128, 255, 255,
                        ))
                        .border_color(tiny_skia::Color::from_rgba8(255, 0, 0, 255))
                        .border_radius(Radius::SizeEqual(10))
                    },
                ),
            )
        ],
    ].fill()
}

fn page() -> impl View<W> {
    let mut widget = create_signal(WidgetTab::Container);
    // let select_widget =
    //     Select::vertical(widget,
    // WidgetTab::each().collect::<Vec<_>>().inert());

    let select_widget = Container::new(
        Flex::col(
            WidgetTab::each()
                .map(|w| {
                    Button::new(w.to_string())
                        .on_click(move || {
                            widget.set(w);
                        })
                        .el()
                })
                .collect::<Vec<_>>(),
        )
        .gap(5u32)
        .fill(),
    )
    .padding(5u32)
    .width(Length::Shrink)
    .height(Length::fill());

    let widget_view = Container::new(
        dynamic(move || match widget.get() {
            WidgetTab::Container => container().into_el(),
            WidgetTab::Button => Button::new("Some button text").el(),
            WidgetTab::Canvas => Label::new("TODO").el(),
            WidgetTab::Checkbox => Checkbox::new(true).el(),
            WidgetTab::Label => Label::new("Some text").el(),
        })
        .el(),
    );

    let page = row![select_widget, col![widget_view].fill()].center().fill();

    page
}

fn main() {
    env_logger::init();

    let output_settings = OutputSettingsBuilder::new().build();

    let mut window = Window::new("Widget gallery", &output_settings);

    let mut display =
        SimulatorDisplay::<Rgb888>::new(Size::new(640, 360).into());

    window.update(&display);

    let mut ui = UI::new(
        Theme::<tiny_skia::Color>::default(),
        TinySkiaRenderer::new(display.bounding_box().size.into()),
    )
    .with_page(SinglePage, page)
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

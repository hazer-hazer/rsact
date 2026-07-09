use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{Dimensions, RgbColor},
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rsact_ui::{event::simulator::simulator_single_encoder, prelude::*};
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
    let border_color = tiny_skia::Color::from_rgba8(0, 128, 255, 255);
    let background_color = tiny_skia::Color::from_rgba8(255, 128, 0, 255);
    let content_color = tiny_skia::Color::from_rgba8(0, 255, 128, 255);

    (
        (
            "Container is a widget with a single child. You can set padding, border and its radius, background color, and alignment of the child.".Container().fill(),

            (
                "Padding [top 5px, right 10, bottom 15, left 20]",
                Space::col(10),
                Edge::new()
                    .size(Size::new_equal(50))
                    .style(move |base, _| {
                        base.background_color(content_color).border_color(border_color)
                    })
                    .Container()
                    .style(move |base, _| base.background_color(background_color))
                    .padding(Padding::new(5, 10, 15, 20))
            ).Col().fill(),

            (
                "Border [width 10px, color red, radius 10px]",
                Space::col(10),
                Edge::new()
                    .size(Size::new_equal(50))
                    .Container()
                    .border_width(10)
                    .style(
                        move |base, _| {
                            base.background_color(background_color)
                            .border_color(border_color)
                            .border_radius(Radius::SizeEqual(10))
                        },
                    )
            ).Col().fill(),
        ).Row().fill(),

        (
            "Alignment [horizontal center, vertical end]".Container().fill(),

            Edge::new()
                .size(Size::new_equal(50))
                .style(move |base, _| {
                    base.background_color(content_color)
                })
                .Container()
                .style(move |base, _| base.background_color(background_color).border_color(border_color))
                .border_width(5)
                .horizontal_align(Align::Center)
                .vertical_align(Align::End)
                .size(Size::new_equal(100)),
        ).Row().fill()
    ).Col().fill()
}

fn page() -> impl View<W> {
    let mut widget = create_signal(WidgetTab::Checkbox);
    // let select_widget =
    //     Select::vertical(widget,
    // WidgetTab::each().collect::<Vec<_>>().inert());

    let select_widget = WidgetTab::each()
        .map(|w| {
            Button::new(w.to_string())
                .on_click(move || {
                    widget.set(w);
                })
                .into_el()
        })
        .collect::<Vec<_>>()
        .Col()
        .gap(5u32)
        .fill()
        .Container()
        .padding(5u32)
        .width_shrink()
        .height_fill();

    let widget_view = dynamic(move || match widget.get() {
        WidgetTab::Container => container().into_el(),
        WidgetTab::Button => Button::new("Some button text").into_el(),
        WidgetTab::Canvas => Label::new("TODO").el(),
        WidgetTab::Checkbox => Checkbox::new(true).el(),
        WidgetTab::Label => Label::new("Some text").el(),
    })
    .Container()
    .fill();

    let page = (select_widget, widget_view).Row().center().fill();

    page
}

fn main() {
    env_logger::init();

    let output_settings = OutputSettingsBuilder::new().build();

    let mut window = Window::new("Widget gallery", &output_settings);

    let mut display =
        SimulatorDisplay::<Rgb888>::new(Size::new(640, 360).into());

    window.set_max_fps(9999999);
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

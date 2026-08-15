use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{Dimensions, RgbColor},
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rsact_render::output::MapColor;
use rsact_ui::{event::simulator::simulator_single_encoder, prelude::*};
use std::{
    fmt::Display,
    time::{Duration, Instant},
};

type Color = tiny_skia::Color;
/// The layer split's tiny-skia stack: tiny-skia rasterizes to coverage, our
/// pixmap blitter blends it. Spelled out rather than aliased in the library —
/// the three parameters are the architecture.
type Skia = RasterRenderer<TinySkiaRasterizer, PixmapBlitter, Unbounded>;
type W = Wtf<Skia, SinglePage, Theme<Color>, ()>;

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
                // WS5.5: `border_width` is a STYLE property, not a layout one.
                // It paints inside the box rather than reserving space, so add
                // padding if it must not overlap content.
                Edge::new()
                    .size(Size::new_equal(50))
                    .Container()
                    .style(
                        move |base, _| {
                            base.background_color(background_color)
                            .border_color(border_color)
                            .border_width(10)
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
                .style(move |base, _| base.background_color(background_color).border_color(border_color).border_width(5))
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
        WidgetTab::Canvas => Label::new("TODO").into_el(),
        WidgetTab::Checkbox => Checkbox::new(true).into_el(),
        WidgetTab::Label => Label::new("Some text").into_el(),
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

    let viewport: Size = display.bounding_box().size.into();
    // The pixmap is the application's — rsact borrows it (WS6.4d).
    let mut renderer = Skia::with_blitter(
        TinySkiaRasterizer::new(),
        viewport,
        PixmapBlitter::new(
            viewport,
            tiny_skia::Pixmap::new(viewport.width, viewport.height).unwrap(),
        ),
    );

    let mut ui = UI::new(Theme::<tiny_skia::Color>::default(), viewport)
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

        {
            let mut frame = ui.start_frame(&mut renderer);
            while frame.render(&mut renderer).is_some() {
                let (parked, blitter) = renderer.detach();
                let (pixmap, at) = blitter.into_pixmap();
                flush(&mut display, &pixmap, at);
                renderer = parked.attach(PixmapBlitter::new(viewport, pixmap));
            }
        }
        window.update(&display);
    }
}

/// Flush a detached pixmap to the display.
///
/// The tiny-skia mirror: rsact hands back a `Pixmap` sized to the region and the
/// rect it belongs at, and what happens next is the application's. Here a
/// simulator window; it could equally be `pixmap.encode_png(..)`, which is why
/// this backend keeps a real `Pixmap` rather than lowering to an embedded color
/// on the way out.
fn flush<D: DrawTarget<Color = Rgb888>>(
    display: &mut D,
    pixmap: &tiny_skia::Pixmap,
    at: Rect,
) {
    let _ = display.fill_contiguous(
        &embedded_graphics::primitives::Rectangle::new(
            at.top_left.into(),
            at.size.into(),
        ),
        pixmap.pixels().iter().map(|p| p.map_color()),
    );
}

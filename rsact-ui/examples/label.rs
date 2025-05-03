use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{Dimensions, RgbColor},
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rand::{Rng, random, rng, thread_rng};
use rsact_reactive::prelude::*;
use rsact_ui::{
    el::El,
    event::simulator::simulator_single_encoder,
    font::FontImport,
    layout::Align,
    page::id::SinglePage,
    prelude::{
        Button, Checkbox, Container, Flex, IntoInert, Length, Select,
        SignalMap, Size, Slider, Text, TextStyle, create_signal,
    },
    render::color::RgbExt,
    style::{NullStyler, WidgetStylist},
    ui::UI,
    widget::{BlockModelWidget, SizedWidget, Widget, WidgetCtx},
};
use u8g2_fonts::FontRenderer;

fn main() {
    let output_settings = OutputSettingsBuilder::new()
        .max_fps(10000)
        // .theme(embedded_graphics_simulator::BinaryColorTheme::OledWhite)
        .build();

    let mut window = Window::new("FLEX", &output_settings);

    let mut display =
        SimulatorDisplay::<Rgb888>::new(Size::new(1280, 720).into());

    window.update(&display);

    // TODO: Reactive axis!?

    let page = Container::new("Hello").fill().el();

    static DEFAULT_FONT: FontRenderer =
        FontRenderer::new::<u8g2_fonts::fonts::u8g2_font_profont22_mf>();

    let mut ui = UI::new_with_buffer_renderer(
        display.bounding_box().size.inert(),
        NullStyler,
        Rgb888::WHITE,
    )
    .auto_focus()
    .on_exit(|| std::process::exit(0))
    .with_page(SinglePage, page)
    .with_default_font(FontImport::fixed_u8g2(&DEFAULT_FONT));

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

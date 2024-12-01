use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rsact_encoder::widget::encoder_keyboard::EncoderKeyboard;
use rsact_ui::{
    embedded_graphics::{pixelcolor::Rgb888, prelude::Dimensions as _},
    event::simulator::simulator_single_encoder,
    page::id::SinglePage,
    prelude::{
        create_signal, Button, Container, Flex, IntoInert as _, Length,
        MonoText, Scrollable, SignalMap, Size,
    },
    render::draw_target::{AntiAliasing, LayeringRendererOptions},
    style::NullStyler,
    ui::UI,
    widget::{SizedWidget, Widget as _},
};

fn main() {
    let output_settings =
        OutputSettingsBuilder::new().max_fps(10000).scale(3).build();

    let mut window = Window::new("SANDBOX", &output_settings);

    let mut display =
        SimulatorDisplay::<Rgb888>::new(Size::new(480, 270).into());

    window.update(&display);

    let value = create_signal(String::new());

    let page = Flex::col([
        Container::new(MonoText::new(
            value.map(|value| format!("Value: {value}")),
        ))
        .center()
        .height(40u32)
        .fill_width()
        .el(),
        EncoderKeyboard::row(value).fill_width().el(),
        // Scrollable::horizontal(Flex::row(
        //     ('a'..='z')
        //         .map(|char| Button::new(char).on_click(move || {}).el())
        //         .collect::<Vec<_>>(),
        // ))
        // .fill_width()
        // .el(),
    ])
    .fill();

    let mut ui = UI::new(display.bounding_box().size.inert(), NullStyler)
        .auto_focus()
        .with_page(SinglePage, page.el())
        .with_renderer_options(
            LayeringRendererOptions::new().anti_aliasing(AntiAliasing::Enabled),
        )
        .on_exit(|| std::process::exit(0));

    loop {
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

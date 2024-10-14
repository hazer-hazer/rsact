use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{Dimensions as _, RgbColor},
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rsact_reactive::prelude::*;
use rsact_ui::{
    event::{message::Message, simulator::simulator_single_encoder},
    layout::size::Size,
    style::accent::AccentStyler,
    ui::UI,
    widget::{
        bar::Bar, button::Button, flex::Flex, knob::Knob, mono_text::MonoText,
        SizedWidget, Widget as _,
    },
};
use std::time::{Duration, Instant};

fn main() {
    let output_settings = OutputSettingsBuilder::new().scale(1).build();

    let mut window = Window::new("SANDBOX", &output_settings);

    let mut display =
        SimulatorDisplay::<Rgb888>::new(Size::new(480, 270).into());

    window.update(&display);

    // let page1 = Flex::row(vec![Button::new("kek")
    //     .on_click(|| {
    //         println!("Go to page 2");
    //         Some(Message::GoTo(2))
    //     })
    //     .el()])
    // .fill()
    // .el();

    // let page2 = Flex::row(vec![Button::new("lol")
    //     .on_click(|| {
    //         println!("Go to page 1");
    //         Some(Message::GoTo(1))
    //     })
    //     .el()])
    // .fill()
    // .el();

    let page = Flex::row(vec![
        Bar::horizontal(127u8).el(),
        Knob::new(127u8).size(50).el(),
    ])
    .fill();

    let mut ui = UI::single_page(
        page,
        display.bounding_box().size,
        AccentStyler::new(Rgb888::RED),
    )
    .on_exit(|| std::process::exit(0));

    ui.current_page().auto_focus();
    // ui.page(2).auto_focus();

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
        ui.draw(&mut display).unwrap();

        window.update(&display);
    }
}

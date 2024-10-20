use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{Dimensions, Point, Primitive, RgbColor},
    Drawable, Pixel,
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rsact_reactive::prelude::*;
use rsact_ui::{
    event::simulator::simulator_single_encoder,
    layout::size::{PointExt, Size},
    prelude::Color,
    render::{
        draw_target::{AntiAliasing, LayeringRendererOptions},
        line::line_aa,
    },
    style::accent::AccentStyler,
    ui::UI,
    value::RangeU8,
    widget::{bar::Bar, flex::Flex, knob::Knob, SizedWidget, Widget as _},
};
use std::time::{Duration, Instant};

fn main() {
    let output_settings =
        OutputSettingsBuilder::new().scale(3).max_fps(10000).build();

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

    let knob_value = use_signal(RangeU8::new_full_range(127));

    let page = Flex::row(vec![
        Bar::horizontal(knob_value).el(),
        Knob::new(knob_value).size(50).el(),
    ])
    .fill();

    let mut ui = UI::single_page(
        page.el(),
        display.bounding_box().size,
        AccentStyler::new(Rgb888::RED),
    )
    .on_exit(|| std::process::exit(0));
    // .with_renderer_options(
    //     LayeringRendererOptions::new()
    //         .anti_aliasing(AntiAliasing::gaussian(5, 0.4)),
    // );

    ui.current_page().auto_focus();
    // ui.page(2).auto_focus();

    let start = Point::new(15, 15);
    let end = Point::new(200, 200);
    let width = 10;
    let color = Rgb888::BLACK;
    let eg_line = embedded_graphics::primitives::Line::new(start, end)
        .into_styled(
            embedded_graphics::primitives::PrimitiveStyleBuilder::new()
                .stroke_color(color)
                .stroke_width(width)
                .build(),
        );

    let second_line = (start.add_x(50), end.add_x(50));

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

        eg_line.draw(&mut display).unwrap();

        line_aa(second_line.0, second_line.1, width, |point, blend| {
            let color = color.mix(blend, Rgb888::WHITE);
            Pixel(point, color).draw(&mut display).unwrap();
        });

        Pixel(start, Rgb888::RED).draw(&mut display).unwrap();
        Pixel(end, Rgb888::RED).draw(&mut display).unwrap();

        Pixel(second_line.0, Rgb888::RED).draw(&mut display).unwrap();
        Pixel(second_line.1, Rgb888::RED).draw(&mut display).unwrap();

        window.update(&display);
    }
}

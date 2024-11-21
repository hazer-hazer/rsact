use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{Dimensions, Point, RgbColor},
    primitives::{Primitive, PrimitiveStyleBuilder},
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rsact_ui::{
    anim::Anim,
    event::{message::MessageQueue, simulator::simulator_single_encoder},
    page::id::SinglePage,
    prelude::{create_memo, IntoInert, ReadSignal, SignalMap, Size},
    render::primitives::circle::Circle,
    style::NullStyler,
    ui::UI,
    widget::{
        canvas::{Canvas, DrawCommand, DrawQueue},
        SizedWidget, Widget,
    },
};
use std::{
    process,
    time::{SystemTime, UNIX_EPOCH},
};

fn main() {
    let output_settings = OutputSettingsBuilder::new().max_fps(10000).build();

    let mut window = Window::new("SANDBOX", &output_settings);

    let mut display =
        SimulatorDisplay::<Rgb888>::new(Size::new(480, 270).into());

    window.update(&display);

    let queue = MessageQueue::new();

    let canvas_queue = DrawQueue::new();
    let page = Canvas::new(canvas_queue).fill().el();

    let mut ui = UI::new(display.bounding_box().size.inert(), NullStyler)
        .on_exit(|| process::exit(0))
        .with_page(SinglePage, page)
        .with_queue(queue);

    let mut anim = queue.anim(
        Anim::new()
            .duration(1_000)
            .delay(2_000)
            .cycles(2)
            .easing(rsact_ui::anim::easing::Easing::EaseInBack)
            .direction(rsact_ui::anim::AnimDir::Alternate),
    );

    canvas_queue.draw(anim.value.map(move |anim_value| {
        let point = Point::new((anim_value * 250.0) as i32, 15);

        println!("Anim value: {anim_value}, point: {point}");

        vec![
            // DrawCommand::Clear(Rgb888::WHITE),
            Circle::new(point, 50)
                .into_styled(
                    PrimitiveStyleBuilder::new()
                        .fill_color(Rgb888::BLACK)
                        .stroke_color(Rgb888::BLACK)
                        .stroke_width(1)
                        .build(),
                )
                .into(),
        ]
    }));

    anim.start();

    loop {
        ui.tick_time_std()
            .tick(window.events().filter_map(simulator_single_encoder));
        ui.draw(&mut display);

        window.update(&display);
    }
}

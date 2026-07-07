use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{Angle, Dimensions, Point, RgbColor, WebColors},
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rsact_ui::{
    anim::Anim,
    event::{message::UiQueue, simulator::simulator_single_encoder},
    page::id::SinglePage,
    prelude::{DrawStyle, IntoInert, ReadSignal, Renderer, Size},
    style::theme::Theme,
    ui::UI,
    widget::{SizedWidget, Widget, canvas::Canvas, ctx::*},
};
use std::{
    process,
    time::{SystemTime, UNIX_EPOCH},
};

fn main() {
    env_logger::init();

    let output_settings = OutputSettingsBuilder::new().build();

    let mut window = Window::new("SANDBOX", &output_settings);

    let mut display =
        SimulatorDisplay::<Rgb888>::new(Size::new(480, 270).into());

    window.update(&display);

    let queue = UiQueue::new();

    let mut circle_anim = queue.anim(
        Anim::new()
            .duration(1_000)
            .delay(2_000)
            .cycles(2)
            .easing(rsact_ui::anim::easing::Easing::EaseInBack)
            .direction(rsact_ui::anim::AnimDir::Alternate),
    );

    let mut loader_anim = queue.anim(
        Anim::new()
            .infinite()
            .easing(rsact_ui::anim::easing::Easing::Linear)
            .direction(rsact_ui::anim::AnimDir::Alternate),
    );

    // Immediate-mode Canvas: the closure re-issues the whole scene every frame,
    // reading the anim value memos so the render observer follows them
    // reactively (WS1b b.1). `renderer` is already clipped to the Canvas rect.
    let circle_value = circle_anim.value;
    let loader_value = loader_anim.value;
    let page = Canvas::new(move |renderer| {
        let circle_v = circle_value.get();
        renderer.circle(
            Point::new((circle_v * 250.0) as i32, 15),
            50,
            &DrawStyle::default()
                .fill(Rgb888::BLACK)
                .stroke(Rgb888::BLACK)
                .stroke_width(1),
        )?;

        let loader_v = loader_value.get();
        renderer.arc(
            Point::new(150, 100),
            50,
            Angle::from_degrees(360.0 * loader_v),
            Angle::from_degrees(360.0 * loader_v),
            &DrawStyle::default()
                .stroke(Rgb888::CSS_PURPLE)
                .stroke_width(10),
        )?;

        Ok(())
    })
    .fill()
    .el();

    let mut ui = UI::new_eg(
        display.bounding_box().size.inert(),
        Theme::default(),
        Rgb888::WHITE,
    )
    .on_exit(|| process::exit(0))
    .with_page(SinglePage, page)
    .with_queue(queue);

    circle_anim.start();
    loader_anim.start();

    loop {
        ui.tick_time_std()
            .tick(window.events().filter_map(simulator_single_encoder));
        ui.render(&mut display);

        window.update(&display);
    }
}

use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{Angle, Dimensions, Point, RgbColor, WebColors},
    primitives::{Primitive, PrimitiveStyleBuilder},
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rsact_ui::{
    anim::Anim,
    event::{message::UiQueue, simulator::simulator_single_encoder},
    page::id::SinglePage,
    prelude::{IntoInert, ReadSignal, SignalMap, Size, create_memo},
    render::{
        draw_target::LayeringRendererOptions,
        primitives::{arc::Arc, circle::Circle},
    },
    style::NullStyler,
    ui::UI,
    widget::{
        SizedWidget, Widget, WidgetCtx,
        canvas::{Canvas, DrawCommand, DrawQueue},
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

    let queue = UiQueue::new();

    let canvas_queue = DrawQueue::new();
    let page = Canvas::new(canvas_queue).fill().el();

    let mut ui =
        UI::new_with_buffer_renderer(
            display.bounding_box().size.inert(),
            NullStyler,
        )
        .on_exit(|| process::exit(0))
        .with_page(SinglePage, page)
        .with_renderer_options(LayeringRendererOptions::new().anti_aliasing(
            rsact_ui::render::draw_target::AntiAliasing::Enabled,
        ))
        .with_queue(queue);

    let mut circle_anim = queue.anim(
        Anim::new()
            .duration(1_000)
            .delay(2_000)
            .cycles(2)
            .easing(rsact_ui::anim::easing::Easing::EaseInBack)
            .direction(rsact_ui::anim::AnimDir::Alternate),
    );

    canvas_queue.draw(circle_anim.value.map(move |anim_value| {
        vec![
            // DrawCommand::Clear(Rgb888::WHITE),
            Circle::new(Point::new((anim_value * 250.0) as i32, 15), 50)
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

    let mut loader_anim = queue.anim(
        Anim::new()
            .infinite()
            .easing(rsact_ui::anim::easing::Easing::Linear)
            .direction(rsact_ui::anim::AnimDir::Alternate),
    );

    canvas_queue.draw(loader_anim.value.map(move |anim_value| {
        vec![
            Arc::new(
                Point::new(150, 100),
                50,
                Angle::from_degrees(360.0 * anim_value),
                Angle::from_degrees(360.0 * anim_value),
            )
            .into_styled(
                PrimitiveStyleBuilder::new()
                    .stroke_color(Rgb888::CSS_PURPLE)
                    .stroke_width(10)
                    .build(),
            )
            .into(),
        ]
    }));

    circle_anim.start();
    loader_anim.start();

    loop {
        ui.tick_time_std()
            .tick(window.events().filter_map(simulator_single_encoder));
        ui.draw(&mut display);

        window.update(&display);
    }
}

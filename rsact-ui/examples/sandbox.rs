use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{Angle, Dimensions, Point, Primitive, RgbColor},
    primitives::{PrimitiveStyleBuilder, StyledDrawable as _},
    transform::Transform as _,
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
        alpha::StyledAlphaDrawable as _,
        draw_target::{AntiAliasing, LayeringRendererOptions},
        primitives::{arc::Arc, circle::Circle, line::Line, polygon::Polygon},
    },
    style::accent::AccentStyler,
    ui::UI,
    value::RangeU8,
    widget::{bar::Bar, flex::Flex, knob::Knob, SizedWidget, Widget as _},
};
use std::{
    f32::consts::PI,
    time::{Duration, Instant},
};

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

    let line_style =
        embedded_graphics::primitives::PrimitiveStyleBuilder::new()
            .stroke_color(Rgb888::BLACK)
            .stroke_width(4)
            .build();
    let line = Line::new(Point::new(15, 15), Point::new(200, 200));

    let circle_style = PrimitiveStyleBuilder::new()
        .stroke_color(Rgb888::BLACK)
        .fill_color(Rgb888::RED)
        .stroke_width(2)
        .build();

    let circle = Circle::new(Point::new(100, 100), 50);

    let arc_style = PrimitiveStyleBuilder::new()
        .stroke_color(Rgb888::BLACK)
        .stroke_width(1)
        .fill_color(Rgb888::RED)
        .build();
    let arc = Arc::new(
        Point::new(300, 100),
        50,
        Angle::zero(),
        Angle::from_degrees(215.0),
    );

    // let polygon_edges = 5;
    // let polygon_center = Point::new(50, 200);
    // let polygon_radius = 25;
    // let polygon_vertices = (0..360).step_by(360 / polygon_edges).map(|angle| {
    //     let point = (angle as f32 * PI / 180.0).sin_cos();
    //     polygon_center
    //         .add_x_round(point.1 * polygon_radius as f32)
    //         .add_y_round(point.0 * polygon_radius as f32)
    // });
    let polygon_vertices = vec![
        Point::new(125, 200),
        Point::new(150, 220),
        Point::new(130, 240),
        Point::new(0, 260),
        Point::new(150, 250),
    ];
    let polygon_style = PrimitiveStyleBuilder::new()
        .stroke_width(1)
        .stroke_color(Rgb888::BLACK)
        .fill_color(Rgb888::MAGENTA)
        .build();
    let polygon = Polygon::new(polygon_vertices);

    // println!("{:#?}", polygon.primitive.lines().collect::<Vec<_>>());

    let line = Line::new(Point::new(50, 50), Point::new(150, 100));

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

        let drawn = ui.draw(&mut display);

        if drawn {
            line.draw_styled(&line_style, &mut display).unwrap();
            line.translate(Point::new(50, 0))
                .draw_styled_alpha(&line_style, &mut display)
                .unwrap();

            circle.draw_styled(&circle_style, &mut display).unwrap();
            circle
                .translate(Point::new(60, 0))
                .draw_styled_alpha(&circle_style, &mut display)
                .unwrap();

            arc.draw_styled(&arc_style, &mut display).unwrap();
            arc.translate(Point::new(60, 0))
                .draw_styled_alpha(&arc_style, &mut display)
                .unwrap();

            polygon.draw_styled(&polygon_style, &mut display).unwrap();
            polygon
                .translate(Point::new(150, 0))
                .draw_styled_alpha(&polygon_style, &mut display)
                .unwrap();
        }

        window.update(&display);
    }
}

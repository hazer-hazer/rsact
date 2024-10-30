use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{Angle, Dimensions, Point, RgbColor, WebColors as _},
    primitives::{PrimitiveStyleBuilder, Rectangle, StyledDrawable as _},
    transform::Transform as _,
    Drawable,
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rsact_reactive::prelude::*;
use rsact_ui::{
    event::simulator::simulator_single_encoder,
    layout::size::Size,
    prelude::{BorderRadius, Color},
    render::{
        alpha::StyledAlphaDrawable as _,
        draw_target::{AntiAliasing, LayeringRendererOptions},
        primitives::{
            arc::Arc, circle::Circle, ellipse::Ellipse, line::Line,
            polygon::Polygon, rounded_rect::RoundedRect, sector::Sector,
        },
    },
    style::accent::AccentStyler,
    ui::UI,
    value::RangeU8,
    widget::{bar::Bar, flex::Flex, knob::Knob, SizedWidget, Widget as _},
};
use std::time::{Duration, Instant};

fn main() {
    let output_settings = OutputSettingsBuilder::new().max_fps(10000).build();

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
    .with_renderer_options(
        LayeringRendererOptions::new().anti_aliasing(AntiAliasing::Enabled),
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
        // .fill_color(Rgb888::RED)
        .build();
    let arc = Arc::new(
        Point::new(220, 100),
        50,
        Angle::zero(),
        Angle::from_degrees(215.0),
    );

    let sector_style = PrimitiveStyleBuilder::new()
        .stroke_color(Rgb888::BLACK)
        .stroke_width(5)
        .fill_color(Rgb888::CYAN)
        .build();
    let sector = Sector::new(
        Point::new(340, 100),
        50,
        Angle::zero(),
        Angle::from_degrees(215.0),
    );

    let ellipse_style = PrimitiveStyleBuilder::new()
        .stroke_color(Rgb888::BLACK)
        .stroke_width(5)
        .fill_color(Rgb888::RED)
        .build();
    let ellipse = Ellipse::new(Point::new(250, 50), Size::new(50, 25));

    let rounded_rect_style = PrimitiveStyleBuilder::new()
        .fill_color(Rgb888::CSS_MEDIUM_SEA_GREEN)
        .stroke_color(Rgb888::BLACK)
        .stroke_width(1)
        .build();
    let rounded_rect = RoundedRect::new(
        Rectangle::new(
            Point::new(320, 200),
            embedded_graphics::geometry::Size::new(60, 40),
        ),
        BorderRadius::new_equal(10.into()),
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
            embedded_graphics::primitives::Arc::new(
                Point::new(150, 150),
                50,
                Angle::zero(),
                Angle::from_degrees(256.0),
            )
            .draw_styled(
                &PrimitiveStyleBuilder::new()
                    .stroke_color(Rgb888::RED)
                    .stroke_width(1)
                    .fill_color(Rgb888::CYAN)
                    .build(),
                &mut display,
            )
            .unwrap();

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

            sector.draw_styled(&sector_style, &mut display).unwrap();
            sector
                .translate(Point::new(60, 0))
                .draw_styled_alpha(&sector_style, &mut display)
                .unwrap();

            ellipse.draw_styled(&ellipse_style, &mut display).unwrap();
            ellipse
                .translate(Point::new(60, 0))
                .draw_styled_alpha(&ellipse_style, &mut display)
                .unwrap();

            polygon.draw_styled(&polygon_style, &mut display).unwrap();
            polygon
                .translate(Point::new(150, 0))
                .draw_styled_alpha(&polygon_style, &mut display)
                .unwrap();

            rounded_rect
                .draw_styled(&rounded_rect_style, &mut display)
                .unwrap();
            rounded_rect
                .translate(Point::new(
                    rounded_rect.rect.size.width as i32 + 10,
                    0,
                ))
                .draw_styled_alpha(&rounded_rect_style, &mut display)
                .unwrap();
        }

        window.update(&display);
    }
}

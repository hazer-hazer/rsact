use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{Angle, Dimensions, Point, RgbColor, WebColors as _},
    primitives::{PrimitiveStyleBuilder, Rectangle, StyledDrawable as _},
    transform::Transform as _,
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rsact_ui::{
    event::simulator::simulator_single_encoder,
    layout::size::Size,
    page::id::SinglePage,
    prelude::{BorderRadius, IntoInert},
    render::{
        AntiAliasing, RendererOptions,
        alpha::StyledAlphaDrawable as _,
        primitives::{
            arc::Arc, circle::Circle, ellipse::Ellipse, line::Line,
            polygon::Polygon, rounded_rect::RoundedRect, sector::Sector,
        },
    },
    style::accent::AccentStyler,
    ui::UI,
    widget::{SizedWidget, Widget as _, flex::Flex},
};
use std::time::{Duration, Instant};

fn main() {
    let output_settings =
        OutputSettingsBuilder::new().max_fps(10000).scale(3).build();

    let mut window = Window::new("SANDBOX", &output_settings);

    let mut display =
        SimulatorDisplay::<Rgb888>::new(Size::new(480, 270).into());

    window.update(&display);

    let page = Flex::row([]).fill();

    let mut ui = UI::new_with_buffer_renderer(
        display.bounding_box().size.inert(),
        AccentStyler::new(Rgb888::RED),
        Rgb888::WHITE,
    )
    .with_page(SinglePage, page.el())
    .with_renderer_options(
        RendererOptions::new().anti_aliasing(AntiAliasing::Enabled),
    )
    .on_exit(|| std::process::exit(0));

    let line_style =
        embedded_graphics::primitives::PrimitiveStyleBuilder::new()
            .stroke_color(Rgb888::BLACK)
            .stroke_width(4)
            .build();
    let line = Line::new(Point::new(50, 50), Point::new(150, 100));

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

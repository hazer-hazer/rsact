use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{Angle, Dimensions, Point, Primitive, RgbColor},
    primitives::{PrimitiveStyleBuilder, StyledDrawable as _},
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
        arc::{arc_aa, circle_aa},
        draw_target::{AntiAliasing, LayeringRendererOptions},
        line::{line_aa, Line},
        polygon::{polygon_aa, Polygon},
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

    let start = Point::new(15, 15);
    let end = Point::new(200, 200);
    let width = 4;
    let color = Rgb888::BLACK;
    let eg_line = embedded_graphics::primitives::Line::new(start, end)
        .into_styled(
            embedded_graphics::primitives::PrimitiveStyleBuilder::new()
                .stroke_color(color)
                .stroke_width(width)
                .build(),
        );

    let second_line = (start.add_x(50), end.add_x(50));

    let circle_center = Point::new(100, 100);
    let circle_radius = 25;
    let circle_stroke_width = 2;
    let circle_stroke_color = Rgb888::BLACK;
    let circle_fill_color = Rgb888::RED;
    let circle = embedded_graphics::primitives::Circle::new(
        circle_center - Point::new_equal(circle_radius as i32),
        circle_radius * 2,
    )
    .into_styled(
        PrimitiveStyleBuilder::new()
            .stroke_color(circle_stroke_color)
            .fill_color(circle_fill_color)
            .stroke_width(circle_stroke_width)
            .build(),
    );

    let circle_aa_center = circle_center.add_x(100);

    let arc_center = Point::new(300, 100);
    let arc_radius = 25u32;
    let arc_angle_start = Angle::zero();
    let arc_angle_sweep = Angle::from_degrees(215.0);
    let arc_stroke_color = Rgb888::BLACK;
    let arc_fill_color = Rgb888::RED;
    let arc_stroke_width = 1;
    let arc = embedded_graphics::primitives::Sector::new(
        arc_center - Point::new_equal(arc_radius as i32),
        arc_radius * 2,
        arc_angle_start,
        arc_angle_sweep,
    )
    .into_styled(
        PrimitiveStyleBuilder::new()
            .stroke_color(arc_stroke_color)
            .stroke_width(arc_stroke_width)
            .fill_color(arc_fill_color)
            .build(),
    );

    let arc_aa_point = arc_center.add_x(50);

    // let polygon_edges = 29;
    // let polygon_center = Point::new(425, 100);
    // let polygon_radius = 25;
    // let polygon_vertices = (0..360).step_by(360 / polygon_edges).map(|angle| {
    //     let point = (angle as f32 * PI / 180.0).sin_cos();
    //     polygon_center
    //         .add_x_round(point.1 * polygon_radius as f32)
    //         .add_y_round(point.0 * polygon_radius as f32)
    // });
    let polygon_vertices = vec![
        Point::new(425, 100),
        Point::new(450, 120),
        Point::new(430, 160),
        Point::new(300, 200),
        Point::new(450, 150),
    ];
    let polygon = Polygon::new(polygon_vertices).into_styled(
        PrimitiveStyleBuilder::new()
            .stroke_width(1)
            .stroke_color(Rgb888::BLACK)
            .fill_color(Rgb888::MAGENTA)
            .build(),
    );

    // println!("{:#?}", polygon.primitive.lines().collect::<Vec<_>>());

    let line = Line::new(Point::new(230, 250), Point::new(320, 150));

    let point = ((line.start + line.end) / 2).add_y(1);
    println!("Point: {point}");
    println!("Dist to line: {}", line.dist_to(point));
    let dx = (line.end.x - line.start.x).pow(2);
    let dy = (line.end.y - line.start.y).pow(2);
    println!("Same as {}?", (dx as f32 / line.len_sq() as f32).sqrt());

    let line_styled =
        embedded_graphics::primitives::Line::new(line.start, line.end)
            .into_styled(
                PrimitiveStyleBuilder::new()
                    .stroke_color(Rgb888::BLACK)
                    .stroke_width(1)
                    .build(),
            );

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
            eg_line.draw(&mut display).unwrap();

            line_aa(second_line.0, second_line.1, width, |point, blend| {
                let color = display.get_pixel(point).mix(blend, color);
                Pixel(point, color).draw(&mut display).unwrap();
            });

            // line_styled.draw(&mut display).unwrap();
            // Pixel(point, Rgb888::RED).draw(&mut display).unwrap();
            line_aa(line.start, line.end, 1, |point, blend| {
                let color = display.get_pixel(point).mix(blend, color);
                Pixel(point, color).draw(&mut display).unwrap();
            });

            circle.draw(&mut display).unwrap();
            circle_aa(
                circle_aa_center,
                circle_radius,
                circle_stroke_width,
                Some(circle_stroke_color),
                // None,
                Some(circle_fill_color),
                |point, color, blend| {
                    let color = display.get_pixel(point).mix(blend, color);
                    Pixel(point, color).draw(&mut display).unwrap();
                },
            );

            arc.draw(&mut display).unwrap();
            arc_aa(
                arc_aa_point,
                arc_radius,
                arc_angle_start,
                arc_angle_sweep,
                Some(arc_stroke_color),
                arc_stroke_width,
                Some(Rgb888::GREEN),
                |point, color, blend| {
                    let color = display.get_pixel(point).mix(blend, color);
                    Pixel(point, color).draw(&mut display).unwrap();
                },
            );

            polygon_aa(&polygon, |point, color, blend| {
                let color = display.get_pixel(point).mix(blend, color);
                Pixel(point, color).draw(&mut display).unwrap();
            });

            // polygon.primitive.lines().for_each(|line| {
            //     embedded_graphics::primitives::Line::new(line.start, line.end)
            //         .draw_styled(
            //             &PrimitiveStyleBuilder::new()
            //                 .stroke_width(polygon.style.stroke_width)
            //                 .stroke_color(polygon.style.stroke_color.unwrap())
            //                 .build(),
            //             &mut display,
            //         )
            //         .unwrap();
            // });

            Pixel(start, Rgb888::RED).draw(&mut display).unwrap();
            Pixel(end, Rgb888::RED).draw(&mut display).unwrap();

            Pixel(second_line.0, Rgb888::RED).draw(&mut display).unwrap();
            Pixel(second_line.1, Rgb888::RED).draw(&mut display).unwrap();
        }

        window.update(&display);
    }
}

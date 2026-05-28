use std::process;

use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rsact_render::{
    image::ImageOwned, primitives::Primitive, tiny_skia::TinySkiaRenderer,
};
use rsact_ui::{
    prelude::*,
    widget::canvas::{Canvas, CanvasImage, DrawCommand, DrawQueue},
};
use tiny_skia::Pixmap;

fn draw_primitive(
    queue: &mut DrawQueue<tiny_skia::Color>,
    primitive: impl Primitive + Copy,
) {
    let fill = tiny_skia::Color::from_rgba8(255, 200, 49, 255);
    let stroke = tiny_skia::Color::from_rgba8(129, 46, 128, 255);

    queue.draw_once(DrawCommand::Primitive(
        primitive.into_kind(),
        DrawStyle::default().stroke(stroke).stroke_width(3),
    ));
    queue.draw_once(DrawCommand::Primitive(
        primitive.translated(Point::new(0, 80)).into_kind(),
        DrawStyle::default().fill(fill),
    ));
    queue.draw_once(DrawCommand::Primitive(
        primitive.translated(Point::new(0, 160)).into_kind(),
        DrawStyle::default().fill(fill).stroke(stroke).stroke_width(3),
    ));
}

fn main() {
    env_logger::init();

    let output_settings = OutputSettingsBuilder::new().scale(1).build();

    let mut window = Window::new("TinySkia", &output_settings);

    let size = Size::new(500, 380);

    let mut display = SimulatorDisplay::<Rgb888>::new(size.into());

    window.update(&display);

    let image_source =
        Pixmap::load_png("rsact-ui/examples/assets/kitty.png").unwrap();

    let mut draw_queue = DrawQueue::new();

    let image = draw_queue.add_image(image_source.into());

    let page = Container::new(Canvas::new(draw_queue)).fill().el();

    let mut ui = UI::new(Theme::default(), TinySkiaRenderer::new(size))
        .no_events()
        .on_exit(|| process::exit(0))
        .with_page(SinglePage, page);

    draw_primitive(
        &mut draw_queue,
        Arc {
            top_left: Point::new(20, 20),
            diameter: 60,
            start: Angle::zero(),
            sweep: Angle::from_degrees(120.0),
        },
    );

    draw_primitive(
        &mut draw_queue,
        RoundedRect {
            rect: Rect {
                top_left: Point::new(100, 20),
                size: Size::new(80, 50),
            },
            corners: CornerRadii {
                top_left: Size::new(0, 0),
                top_right: Size::new(10, 10),
                bottom_right: Size::new(20, 20),
                bottom_left: Size::new(40, 25),
            },
        },
    );

    draw_primitive(
        &mut draw_queue,
        Rect { top_left: Point::new(200, 20), size: Size::new(80, 50) },
    );

    draw_primitive(
        &mut draw_queue,
        Circle { top_left: Point::new(300, 20), diameter: 60 },
    );

    draw_primitive(
        &mut draw_queue,
        Ellipse { top_left: Point::new(380, 20), size: Size::new(60, 20) },
    );

    draw_primitive(
        &mut draw_queue,
        Sector {
            top_left: Point::new(420, 20),
            diameter: 60,
            start: Angle::zero(),
            sweep: Angle::from_degrees(120.0),
        },
    );

    draw_primitive(
        &mut draw_queue,
        Line { from: Point::new(10, 10), to: Point::new(150, 10) },
    );

    draw_queue.draw_once(DrawCommand::Image(CanvasImage::new(
        image,
        Point::new(20, 260),
    )));

    ui.render(&mut display);
    window.show_static(&display);
}

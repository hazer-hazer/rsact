use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rsact_render::{
    image::{DrawImage, ImageOwned},
    primitives::Primitive,
    tiny_skia::TinySkiaRenderer,
};
use rsact_ui::{prelude::*, widget::canvas::Canvas};
use std::process;
use tiny_skia::Pixmap;

/// Draw a `PrimitiveKind` through the renderer's typed methods. The old
/// `DrawCommand` model carried this dispatch inside `Canvas::render`; with the
/// immediate-mode Canvas (WS1b b.1) the draw closure issues primitives itself.
fn draw_kind<R: Renderer>(
    renderer: &mut R,
    kind: PrimitiveKind,
    style: &DrawStyle<R::Color>,
) -> RenderResult {
    match kind {
        PrimitiveKind::Arc(a) => {
            renderer.arc(a.top_left, a.diameter, a.start, a.sweep, style)
        },
        PrimitiveKind::Circle(c) => {
            renderer.circle(c.top_left, c.diameter, style)
        },
        PrimitiveKind::Ellipse(e) => {
            renderer.ellipse(Rect::new(e.top_left, e.size), style)
        },
        PrimitiveKind::Line(l) => renderer.line(l.from, l.to, style),
        PrimitiveKind::Polygon(p) => renderer.polygon(&p.vertices, style),
        PrimitiveKind::Rect(rect) => renderer.rect(rect, style),
        PrimitiveKind::RoundedRect(rr) => {
            renderer.rounded_rect(rr.rect, rr.corners, style)
        },
        PrimitiveKind::Sector(s) => {
            renderer.sector(s.top_left, s.diameter, s.start, s.sweep, style)
        },
    }
}

fn draw_primitive<R: Renderer<Color = tiny_skia::Color>>(
    renderer: &mut R,
    primitive: impl Primitive + Clone,
) -> RenderResult {
    let fill = tiny_skia::Color::from_rgba8(255, 200, 49, 255);
    let stroke = tiny_skia::Color::from_rgba8(129, 46, 128, 255);

    draw_kind(
        renderer,
        primitive.clone().into_kind(),
        &DrawStyle::default().stroke(stroke).stroke_width(3),
    )?;
    draw_kind(
        renderer,
        primitive.translated(Point::new(0, 80)).into_kind(),
        &DrawStyle::default().fill(fill),
    )?;
    draw_kind(
        renderer,
        primitive.translated(Point::new(0, 160)).into_kind(),
        &DrawStyle::default()
            .fill(fill)
            .stroke(stroke)
            .stroke_width(3),
    )?;
    Ok(())
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
    let image: ImageOwned<tiny_skia::Color> = image_source.into();

    // Immediate-mode Canvas: one closure re-issues the whole gallery each
    // frame, drawing straight into the renderer clipped to the Canvas rect
    // (WS1b b.1). No retained command buffer / image storage.
    let page = Container::new(Canvas::new(
        move |renderer: &mut TinySkiaRenderer<tiny_skia::Color>| {
            draw_primitive(
                renderer,
                Arc {
                    top_left: Point::new(20, 20),
                    diameter: 60,
                    start: Angle::zero(),
                    sweep: Angle::from_degrees(120.0),
                },
            )?;
            draw_primitive(
                renderer,
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
            )?;
            draw_primitive(
                renderer,
                Rect { top_left: Point::new(200, 20), size: Size::new(80, 50) },
            )?;
            draw_primitive(
                renderer,
                Circle { top_left: Point::new(300, 20), diameter: 60 },
            )?;
            draw_primitive(
                renderer,
                Ellipse {
                    top_left: Point::new(380, 20),
                    size: Size::new(60, 20),
                },
            )?;
            draw_primitive(
                renderer,
                Sector {
                    top_left: Point::new(420, 20),
                    diameter: 60,
                    start: Angle::zero(),
                    sweep: Angle::from_degrees(120.0),
                },
            )?;
            draw_primitive(
                renderer,
                Line { from: Point::new(10, 10), to: Point::new(150, 10) },
            )?;

            renderer
                .image(DrawImage::new(image.as_ref(), Point::new(20, 260)))?;

            Ok(())
        },
    ))
    .fill()
    .into_el();

    let mut ui = UI::new(Theme::default(), TinySkiaRenderer::new(size))
        .no_events()
        .on_exit(|| process::exit(0))
        .with_page(SinglePage, page);

    ui.render(&mut display);
    window.show_static(&display);
}

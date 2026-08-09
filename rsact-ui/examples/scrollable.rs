use embedded_graphics::{
    pixelcolor::{BinaryColor, Rgb888},
    prelude::{Dimensions, RgbColor},
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rand::{Rng, random, rng};
use rsact_reactive::prelude::*;
use rsact_ui::{event::simulator::simulator_single_encoder, prelude::*};
use u8g2_fonts::FontRenderer;

fn main() {
    env_logger::init();

    let output_settings = OutputSettingsBuilder::new().scale(5).build();

    let mut window = Window::new("FLEX", &output_settings);

    let mut display =
        SimulatorDisplay::<BinaryColor>::new(Size::new(128, 80).into());

    window.update(&display);

    let page = Scrollable::vertical(
        col![
            Button::new("Abcdefghijklmnopqr"),
            Button::new("Abcdefghijklmnopqr"),
            Button::new("Abcdefghijklmnopqr"),
            Button::new("Abcdefghijklmnopqr"),
            Button::new("Abcdefghijklmnopqr"),
            Button::new("Abcdefghijklmnopqr"),
            Button::new("Abcdefghijklmnopqr")
        ]
        .gap(5u32)
        .fill_width(),
    )
    .fill()
    .el();

    let viewport: Size = display.bounding_box().size.into();
    // The framebuffer is the APPLICATION's — rsact never allocates one
    // and never owns one. On a device this would be a `StaticCell` array
    // placed wherever that board wants it (SDRAM, DTCM, a DMA pool).
    let mut renderer = EGRenderer::<Rgb888, AntiAliasingDisabled, _>::new(
        viewport,
        vec![0u32; viewport.area() as usize].into_boxed_slice(),
    );

    let mut ui = UI::new(Theme::default(), viewport)
        .auto_focus()
        .on_exit(|| std::process::exit(0))
        .with_page(SinglePage, page);

    loop {
        ui.tick(
            window
                .events()
                .filter_map(simulator_single_encoder)
                .inspect(|e| println!("Event: {e:?}")),
        );
        if ui.render(&mut renderer) {
            // Take the buffer back, ship what changed, hand it in again.
            // `detach` consumes the renderer and returns a parked one, so the
            // buffer cannot be painted into while this loop holds it.
            let (parked, buf, covers) = renderer.detach();
            ui.with_damage(|rects| {
                for r in rects {
                    flush_rect(&mut display, &buf, covers, *r);
                }
            });
            renderer = parked.attach(buf);
        }

        window.update(&display);
    }
}

/// Flush the damaged rects of a detached framebuffer to the display.
///
/// **This is the application's job, not rsact's** (WS6.4d). rsact renders into
/// a buffer the app lends it and says what changed; where those pixels go, and
/// how, is the app's decision — here an `embedded-graphics` `DrawTarget`, on a
/// device a `CASET`/`RASET` window plus a DMA burst.
///
/// `covers` is the rect the buffer holds (from `detach`), so rows are strided at
/// `covers.size.width`: for a full-frame surface that is the frame width, for a
/// tile it is the tile's own. `dirty` is the sub-rect worth sending.
fn flush_rect<D: DrawTarget<Color = Rgb888>>(
    display: &mut D,
    units: &[u32],
    covers: Rect,
    dirty: Rect,
) {
    let dirty = dirty.intersection(&covers);
    let stride = covers.size.width as usize;
    let _ = display.fill_contiguous(
        &embedded_graphics::primitives::Rectangle::new(
            dirty.top_left.into(),
            dirty.size.into(),
        ),
        dirty.points().map(|p| {
            let col = (p.x - covers.top_left.x) as usize;
            let row = (p.y - covers.top_left.y) as usize;
            <Rgb888 as PackedColor>::as_color(&units[row * stride + col], 0)
        }),
    );
}

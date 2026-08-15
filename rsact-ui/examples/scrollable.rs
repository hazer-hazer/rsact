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
    let mut renderer =
        RasterRenderer::<_, FramebufBlitter<Rgb888, _>, Unbounded>::with_framebuf(
            EgRasterizer,
            viewport,
            // `.leak()` rather than `into_boxed_slice()`: the buffer contract is
            // a `&'static mut` loan (a `StaticCell` on a device), and the
            // owned-`Box` impl went with WS6.4d — a renderer BORROWS a surface.
            vec![0u32; viewport.area() as usize].leak(),
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
        // The one render path: plan the frame, then paint and ship one region
        // at a time. `at` is both what the buffer holds and where it goes —
        // `begin_region` retargets every surface, so a whole framebuffer and a
        // tile are the same shape here.
        {
            let mut frame = ui.start_frame(&mut renderer);
            while frame.render(&mut renderer).is_some() {
                let (parked, buf, at) = renderer.detach();
                flush(&mut display, &buf, at);
                renderer = parked.attach(buf);
            }
        }

        window.update(&display);
    }
}

/// Flush a detached buffer to the display.
///
/// **This is the application's job, not rsact's** (WS6.4d). rsact renders into a
/// buffer the app lends it and hands it back with the rect it holds; where those
/// pixels go, and how, is the app's decision — here an `embedded-graphics`
/// `DrawTarget`, on a device a `CASET`/`RASET` window plus a DMA burst.
///
/// One rect, because `begin_region` retargets every surface: `at` is both what
/// the buffer contains and where it goes, so rows are strided at `at`'s width
/// whether this is a tile or a whole framebuffer.
fn flush<D: DrawTarget<Color = Rgb888>>(
    display: &mut D,
    units: &[u32],
    at: Rect,
) {
    let stride = at.size.width as usize;
    let _ = display.fill_contiguous(
        &embedded_graphics::primitives::Rectangle::new(
            at.top_left.into(),
            at.size.into(),
        ),
        (0..at.size.height as usize).flat_map(|row| {
            (0..stride).map(move |col| {
                <Rgb888 as PackedColor>::as_color(&units[row * stride + col], 0)
            })
        }),
    );
}

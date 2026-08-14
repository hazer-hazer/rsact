use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{Dimensions, RgbColor},
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rsact_ui::{
    col,
    event::simulator::simulator_single_encoder,
    page::id::SinglePage,
    prelude::{IntoInert, Select, create_signal, *},
    row,
    style::theme::Theme,
    ui::UI,
    widget::{SizedWidget, Widget, flex::Flex},
};
use std::time::{Duration, Instant};

fn main() {
    env_logger::init();

    let output_settings = OutputSettingsBuilder::new().scale(3).build();

    let mut window = Window::new("SANDBOX", &output_settings);

    let mut display =
        SimulatorDisplay::<Rgb888>::new(Size::new(480, 270).into());

    window.update(&display);

    let selected = create_signal(0);
    let select = Select::vertical(selected, vec![0, 1, 2, 3].inert());

    let page = row![col![select]].center().fill();

    let viewport: Size = display.bounding_box().size.into();
    // The framebuffer is the APPLICATION's — rsact never allocates one
    // and never owns one. On a device this would be a `StaticCell` array
    // placed wherever that board wants it (SDRAM, DTCM, a DMA pool).
    let mut renderer = EGRenderer::<Rgb888, AntiAliasingDisabled, _>::new(
        viewport,
        vec![0u32; viewport.area() as usize].into_boxed_slice(),
    );

    let mut ui = UI::new(Theme::default(), viewport)
        .with_page(SinglePage, page.el())
        .on_exit(|| std::process::exit(0));

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

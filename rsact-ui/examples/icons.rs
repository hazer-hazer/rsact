use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{Dimensions, RgbColor},
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rsact_render::{eg::renderer::EGRenderer, framebuf::PackedColor};
use rsact_tiny_icons::{IconSet, common::CommonIcon, system::SystemIcon};
use rsact_ui::{
    page::id::SinglePage,
    prelude::{Flex, Icon, IntoInert, Label, Size, View},
    style::theme::Theme,
    ui::UI,
    widget::{SizedWidget, Widget},
};
use std::env;

fn main() {
    env_logger::init();

    let output_settings = OutputSettingsBuilder::new().scale(1).build();

    let mut window = Window::new("SANDBOX", &output_settings);

    let mut display =
        SimulatorDisplay::<Rgb888>::new(Size::new(480, 270).into());

    window.update(&display);

    const ICON_SIZE: u32 = 12;

    let system_icons = SystemIcon::KINDS
        .iter()
        .copied()
        .map(|kind| Icon::new(kind).size(ICON_SIZE).into_el())
        .collect::<Vec<_>>();

    let common_icons = CommonIcon::KINDS
        .iter()
        .copied()
        .map(|kind| Icon::new(kind).size(ICON_SIZE).into_el())
        .collect::<Vec<_>>();

    let mut ui = UI::new(
        Theme::default(),
        EGRenderer::new(
            display.bounding_box().size.into(),
            // The framebuffer is the APPLICATION's — rsact borrows it and gives
            // it back. On a device this would be a `StaticCell` array instead.
            heap_surface::<Rgb888>(display.bounding_box().size.into()),
        )
    ).no_events().with_page(SinglePage,
        Flex::col([
            Label::new("System icons").el(),
            Flex::row(system_icons).wrap(true).gap(5u32).el(),
            Label::new("Common icons").el(),
            Flex::row(common_icons).wrap(true).gap(5u32).el(),
            Label::new(format!("Icons of size {ICON_SIZE}. Auto-generated from Material Design Icons")).el()
        ])
        .center()
        .fill()
        .el(),);

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

    unsafe { env::set_var("EG_SIMULATOR_DUMP", "assets/icons.png") };
    window.show_static(&display);
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

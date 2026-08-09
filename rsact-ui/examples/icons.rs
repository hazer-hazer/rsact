use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{Dimensions, RgbColor},
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rsact_render::eg::{framebuf::PackedColor, renderer::EGRenderer};
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

    if ui.render(&mut renderer) {
        // Take the buffer back, ship what changed, hand it in again.
        let (buf, covers) = renderer.detach().expect("attached");
        ui.with_damage(|rects| {
            for r in rects {
                flush_rect(&mut display, &buf, covers, *r);
            }
        });
        renderer.attach(buf);
    }

    unsafe { env::set_var("EG_SIMULATOR_DUMP", "assets/icons.png") };
    window.show_static(&display);
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

use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{Dimensions, RgbColor},
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rsact_render::eg::renderer::EGRenderer;
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
        EGRenderer::new(display.bounding_box().size.into())
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

    ui.render(&mut display);

    unsafe { env::set_var("EG_SIMULATOR_DUMP", "assets/icons.png") };
    window.show_static(&display);
}

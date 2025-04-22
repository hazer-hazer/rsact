use std::env;

use embedded_graphics::{pixelcolor::Rgb888, prelude::{Dimensions, RgbColor}};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rsact_icons::{common::CommonIcon, system::SystemIcon, IconSet};
use rsact_ui::{
    page::id::SinglePage, prelude::{Flex, Icon, IntoInert, Size, Text}, style::NullStyler, ui::UI, widget::{SizedWidget, Widget}
};

fn main() {
    let output_settings =
        OutputSettingsBuilder::new().scale(1).max_fps(10000).build();

    let mut window = Window::new("SANDBOX", &output_settings);

    let mut display =
        SimulatorDisplay::<Rgb888>::new(Size::new(480, 270).into());

    window.update(&display);

    const ICON_SIZE: u32 = 12;

    let system_icons = SystemIcon::KINDS
        .iter()
        .copied()
        .map(|kind| Icon::new(kind).size(ICON_SIZE).el())
        .collect::<Vec<_>>();

    let common_icons = CommonIcon::KINDS
        .iter()
        .copied()
        .map(|kind| Icon::new(kind).size(ICON_SIZE).el())
        .collect::<Vec<_>>();

    let mut ui = UI::new_with_buffer_renderer(
        display.bounding_box().size.inert(),
        NullStyler,
        Rgb888::WHITE,
    ).no_events().with_page(SinglePage, 
        Flex::col([
            Text::new_inert("System icons").el(),
            Flex::row(system_icons).wrap(true).gap(5u32).el(),
            Text::new_inert("Common icons").el(),
            Flex::row(common_icons).wrap(true).gap(5u32).el(),
            Text::new_inert(format!("Icons of size {ICON_SIZE}. Auto-generated from Material Design Icons")).el()
        ])
        .center()
        .fill()
        .el(),);

    ui.render(&mut display);

    unsafe { env::set_var("EG_SIMULATOR_DUMP", "assets/icons.png") };
    window.show_static(&display);
}

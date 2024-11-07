use std::env;

use embedded_graphics::{pixelcolor::Rgb888, prelude::Dimensions};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rsact_icons::{common::CommonIcon, system::SystemIcon, IconSet};
use rsact_ui::{
    event::NullEvent,
    prelude::{create_memo, Flex, Icon, MonoText, Size},
    style::NullStyler,
    ui::UI,
    widget::{SizedWidget, Widget},
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

    let mut ui: UI<
        rsact_ui::widget::Wtf<
            _,
            NullEvent,
            NullStyler,
            rsact_ui::page::id::SinglePage,
        >,
    > = UI::single_page(
        Flex::col([
            MonoText::new_static("System icons").el(),
            Flex::row(system_icons).wrap(true).gap(5).el(),
            MonoText::new_static("Common icons").el(),
            Flex::row(common_icons).wrap(true).gap(5).el(),
            MonoText::new_static(format!("Icons of size {ICON_SIZE}. Auto-generated from Material Design Icons")).el()
        ])
        .center()
        .fill()
        .el(),
        display.bounding_box().size,
        NullStyler,
    );

    ui.draw(&mut display);

    env::set_var("EG_SIMULATOR_DUMP", "assets/icons.png");
    window.show_static(&display);
}

use std::os::unix::process;

use embedded_graphics::{
    pixelcolor::{BinaryColor, Rgb888},
    prelude::Dimensions,
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rand::random;
use rsact_reactive::memo::{Keyed, NeverEqual};
use rsact_ui::{
    el::El,
    event::simulator::simulator_single_encoder,
    layout::Align,
    page::id::SinglePage,
    prelude::{
        create_signal, BlockStyle, Edge, Flex, IntoInert, MonoText, Select,
        SignalMap, SignalSetter, Size, Slider,
    },
    style::NullStyler,
    ui::UI,
    widget::{SizedWidget, Widget, WidgetCtx},
};

fn random_color_edge<W: WidgetCtx<Color = Rgb888>>() -> El<W> {
    Edge::new()
        .size(Size::new(50.into(), 50.into()))
        .style(|base| {
            base.background_color(Rgb888::new(random(), random(), random()))
        })
        .el()
}

fn main() {
    let output_settings = OutputSettingsBuilder::new()
        .max_fps(10000)
        // .theme(embedded_graphics_simulator::BinaryColorTheme::OledWhite)
        .build();

    let mut window = Window::new("FLEX", &output_settings);

    let mut display =
        SimulatorDisplay::<Rgb888>::new(Size::new(1280, 720).into());

    window.update(&display);

    let children_count = create_signal(10u8);
    let gap = create_signal(Size::new(5, 5));
    let horizontal_align = create_signal(Align::Start);
    let vertical_align = create_signal(Align::Start);

    let mut children = create_signal(vec![]);

    children.setter(children_count, |children, &count| {
        children.resize_with(count as usize, random_color_edge);
    });

    let props = Flex::col([
        MonoText::new(children_count.map(|count| format!("Children: {count}")))
            .el(),
        Slider::horizontal(children_count).el(),
        MonoText::new(gap.map(|gap| format!("Gap: {gap}"))).el(),
        MonoText::new(
            horizontal_align
                .map(|align| format!("Horizontal alignment: {align}")),
        )
        .el(),
        Select::horizontal(
            horizontal_align,
            vec![Align::Start, Align::Center, Align::End].inert(),
        )
        .el(),
        MonoText::new(
            vertical_align.map(|align| format!("Vertical alignment: {align}")),
        )
        .el(),
        Select::horizontal(
            vertical_align,
            vec![Align::Start, Align::Center, Align::End].inert(),
        )
        .el(),
    ])
    .el();

    let flex = Flex::row(children)
        .gap(gap)
        .vertical_align(vertical_align)
        .horizontal_align(horizontal_align)
        .fill()
        .el();

    let page = Flex::row([props, flex]).gap(5u32);

    let mut ui = UI::new(display.bounding_box().size.inert(), NullStyler)
        .auto_focus()
        .on_exit(|| std::process::exit(0))
        .with_page(SinglePage, page);

    loop {
        ui.tick(window.events().filter_map(simulator_single_encoder));
        ui.draw(&mut display);

        window.update(&display);
    }
}

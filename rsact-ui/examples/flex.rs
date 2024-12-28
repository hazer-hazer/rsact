use embedded_graphics::{pixelcolor::Rgb888, prelude::Dimensions};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rand::{random, thread_rng, Rng};
use rsact_reactive::prelude::*;
use rsact_ui::{
    el::El,
    event::simulator::simulator_single_encoder,
    font::FontImport,
    layout::Align,
    page::id::SinglePage,
    prelude::{
        create_signal, Button, Checkbox, Container, Flex, IntoInert, Length,
        Select, SignalMap, Size, Slider, Text, TextStyle,
    },
    render::color::RgbColor,
    style::{NullStyler, WidgetStylist},
    ui::UI,
    widget::{BlockModelWidget, SizedWidget, Widget, WidgetCtx},
};
use u8g2_fonts::FontRenderer;

fn random_size() -> Size<Length> {
    Size::new(
        Length::Fixed(thread_rng().gen_range(25..=100)),
        Length::Fixed(thread_rng().gen_range(25..=100)),
    )
}

fn item<W: WidgetCtx<Color = Rgb888>>(size: Size<Length>) -> El<W>
where
    W::Styler: WidgetStylist<TextStyle<Rgb888>>,
{
    Container::new(Text::new_inert(size))
        .center()
        .size(size)
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

    // TODO: Reactive axis!?

    let gap_x = create_signal(5u8);
    let gap_y = create_signal(5u8);
    let wrap = create_signal(false);
    let horizontal_align = create_signal(Align::Start);
    let vertical_align = create_signal(Align::Start);

    let mut children = create_signal(vec![]);

    let props = Flex::col([
        Text::new(
            children.map(|children| format!("Children: {}", children.len())),
        )
        .el(),
        Text::new_inert("Add item").el(),
        Button::new("Add random size item")
            .on_click(move || {
                children.update(|children| children.push(item(random_size())))
            })
            .el(),
        Button::new("Add fill item")
            .on_click(move || {
                children.update(|children| children.push(item(Size::fill())))
            })
            .el(),
        Text::new(map!(move |gap_x, gap_y| format!("Gap: {gap_x}x{gap_y}")))
            .el(),
        Slider::horizontal(gap_x).el(),
        Slider::horizontal(gap_y).el(),
        Text::new(
            wrap.map(|&wrap| {
                format!("{}", if wrap { "wrap" } else { "no wrap" })
            }),
        )
        .el(),
        Checkbox::new(wrap).el(),
        Text::new(
            horizontal_align
                .map(|align| format!("Horizontal alignment: {align}")),
        )
        .el(),
        Select::horizontal(
            horizontal_align,
            vec![Align::Start, Align::Center, Align::End].inert(),
        )
        .el(),
        Text::new(
            vertical_align.map(|align| format!("Vertical alignment: {align}")),
        )
        .el(),
        Select::horizontal(
            vertical_align,
            vec![Align::Start, Align::Center, Align::End].inert(),
        )
        .el(),
    ])
    .width(350u32)
    .gap(5u32)
    .padding(5u32)
    .el();

    let flex = Container::new(
        Flex::row(children)
            .gap(map!(move |gap_x, gap_y| Size::new(
                (*gap_x) as u32,
                (*gap_y) as u32
            )))
            .wrap(wrap)
            .vertical_align(vertical_align)
            .horizontal_align(horizontal_align)
            .fill(),
    )
    .style(|base| base.background_color(Rgb888::hex(0x636363)))
    .fill()
    .el();

    let page = Flex::row([props, flex]).gap(5u32).fill();

    static DEFAULT_FONT: FontRenderer =
        FontRenderer::new::<u8g2_fonts::fonts::u8g2_font_profont22_mf>();

    let mut ui = UI::new(display.bounding_box().size.inert(), NullStyler)
        .auto_focus()
        .on_exit(|| std::process::exit(0))
        .with_page(SinglePage, page)
        .with_default_font(FontImport::fixed_u8g2(&DEFAULT_FONT));

    loop {
        ui.tick(
            window
                .events()
                .filter_map(simulator_single_encoder)
                .inspect(|e| println!("Event: {e:?}")),
        );
        ui.draw(&mut display);

        window.update(&display);
    }
}

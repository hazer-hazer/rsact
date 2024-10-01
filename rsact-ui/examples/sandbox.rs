use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{Dimensions as _, RgbColor as _, WebColors as _},
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rand::random;
use rsact_core::prelude::*;
use rsact_ui::{
    el::El,
    event::simulator::simulator_single_encoder,
    layout::{
        size::{Length, Size},
        Align,
    },
    style::{
        block::{BorderStyle, BoxStyle},
        NullStyler,
    },
    ui::UI,
    widget::{BoxModelWidget, SizedWidget as _, Widget as _, WidgetCtx},
    widgets::{
        button::{Button, ButtonState},
        container::Container,
        edge::Edge,
        flex::Flex,
        mono_text::MonoText,
        scrollable::{Scrollable, ScrollableState, ScrollbarShow},
        select::Select,
    },
};
use std::time::{Duration, Instant};

fn edge<W: WidgetCtx<Color = Rgb888>>() -> El<W> {
    Edge::new()
        .style(|_| {
            BoxStyle::base().background_color(Rgb888::new(
                random(),
                random(),
                random(),
            ))
        })
        .width(50)
        .height(50)
        .el()
}

fn text_block<W: WidgetCtx>() -> El<W> {
    Container::new(text()).fill().el()
}

fn text<W: WidgetCtx>() -> El<W> {
    MonoText::new("a".to_string()).el()
}

fn main() {
    let rng = rand::thread_rng();

    // let items = [(Length::Fixed(50), 50.into()); 5];
    // let items = [(Length::Div(5), 50.into()), (Length::Div(6), 50.into())];
    // let items = [
    //     (Length::Div(5), 50.into()),
    //     (Length::Div(4), 50.into()),
    //     (Length::Div(3), 50.into()),
    //     (Length::Div(2), 50.into()),
    //     (Length::Div(2), 50.into()),
    //     (Length::Div(1), 50.into()),
    // ];

    // let flexbox = FlexBox {
    //     wrap: true,
    //     size: Size::new(250.into(), 250.into()),
    //     axis: Axis::X,
    //     gap: Size::new(5, 5),
    //     horizontal_align: Align::End,
    //     vertical_align: Align::End,
    //     children: items
    //         .into_iter()
    //         .map(|size| {
    //             Item {
    //                 size: Size::from(size),
    //                 color: Rgb888::new(random(), random(), random()),
    //             }
    //             .el()
    //         })
    //         .collect(),
    //     color: Rgb888::WHITE,
    // };

    let output_settings = OutputSettingsBuilder::new().scale(1).build();

    let mut window = Window::new("SANDBOX", &output_settings);

    let mut display =
        SimulatorDisplay::<Rgb888>::new(Size::new(480, 270).into());

    window.update(&display);

    // let mut items_height = use_signal(50);

    let button_style = |base, state| match state {
        ButtonState { pressed: true, .. } => base,
        ButtonState { .. } => base,
    };

    let items = use_signal(vec![]);
    let add_item = move || {
        items.update(|items| {
            items.push(MonoText::new(items.len().to_string()).el())
        })
    };

    let buttons = Flex::row(vec![
        Button::new("Add")
            .style(button_style)
            .fill()
            .on_click(move || add_item())
            .el(),
        Button::new("Remove")
            .style(button_style)
            .fill()
            .on_click(move || {
                items.update(|items| {
                    items.pop();
                })
            })
            .el(),
    ]);

    for _ in 0..5 {
        add_item()
    }

    // TODO: Fix Flex::row in Scrollable::vertical

    // let slider_value = use_signal(0);
    // let slider = Slider::horizontal(slider_value);

    let select_value = use_signal("Meow");
    let select = Select::horizontal(vec!["Hello, kek, mr kek", "Meow"])
        .use_value(select_value);

    let flexbox = Flex::col(vec![
        // Flex::row(core::array::from_fn::<_, 100, _>(|_| edge()))
        //     .fill()
        //     .wrap(true)
        //     .el(),
        // slider.el(),
        buttons.fill().el(),
        Scrollable::horizontal(
            // MonoText::new(select_value.mapped(ToString::to_string)).el()
            // "weoinoweinfoewfoewofn[ewfe0[fheqw0fenvo0fvei0[fenfc0[ewnfi0jew0[fi0[ewnfi0ewn[fownefnewnfoiwenfoewnfoiewnfoewnfoiewnfoiewnfoiwen"
            Flex::row(vec![
                Button::new("11111111111111111111").el(),
                Button::new("22222222222222222222").el(),
                Button::new("33333333333333333333").el(),
                Button::new("44444444444444444444").el(),
            ]).el()
        ).fill().tracker().el(),
        Flex::row(items).gap(5).wrap(true).el(),
        // .style(|base, state| {
        //     let base = base.show(ScrollbarShow::Auto);

        //     match state {
        //         ScrollableState { active: true, .. } => base
        //             .container(
        //                 BoxStyle::base().border(BorderStyle::base().color(Rgb888::MAGENTA)),
        //             )
        //             .thumb_color(Some(Rgb888::CSS_GRAY))
        //             .track_color(Some(Rgb888::CSS_BROWN)),
        //         ScrollableState { .. } => base,
        //     }
        // })
        // .el(),
        select.fill().el(),
        // Flex::row(items).gap(5).fill().wrap(true).padding(5).el(),
        // Flex::row([edge(), edge()]).fill().el(),
        // Flex::row([text(), text(), text(), text()]).horizontal_align(Align::Center).gap(5).fill().el(),
        // Flex::row([text(), text(), text(), text()]).gap(5).el(),
        // Flex::row([text_block(),text_block(),text_block(),text_block()]).gap(5).fill().el(),
        // Flex::row([text_block(),text_block(),text_block(),text_block()]).gap(5).fill().el(),
    ])
    // .wrap(true)
    .fill();

    let mut ui = UI::new(flexbox, display.bounding_box().size, NullStyler)
        .on_exit(|| std::process::exit(0));

    ui.current_page().auto_focus();

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

        // items.update(move |items| {
        //     items.push(
        //         Edge::new()
        //             .width(Length::Fixed(20))
        //             .height::<u32>(items_height)
        //
        // .with_style(BoxStyle::base().background_color(Rgb888::new(
        //                 random(),
        //                 random(),
        //                 random(),
        //             )))
        //             .el(),
        //     )
        // });

        // thread::sleep(Duration::from_millis(100));

        ui.tick(
            window
                .events()
                .map(simulator_single_encoder)
                .filter_map(|e| e)
                .inspect(|e| println!("Event: {e:?}")),
        );
        ui.draw(&mut display).unwrap();

        window.update(&display);
    }
}

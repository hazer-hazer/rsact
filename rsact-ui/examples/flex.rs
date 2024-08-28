use std::{
    array, thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use embedded_graphics::{pixelcolor::Rgb888, prelude::Dimensions as _};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rand::random;
use rsact_core::{prelude::*, signal::EcoSignal};
use rsact_ui::{
    event::NullEvent,
    layout::{
        size::{Length, Size},
        Align,
    },
    style::BoxStyle,
    ui::UI,
    widget::Widget as _,
    widgets::{edge::Edge, flex::Flex, space::Space},
};

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

    let mut window = Window::new("TEST", &output_settings);

    let mut display =
        SimulatorDisplay::<Rgb888>::new(Size::new(480, 270).into());

    window.update(&display);

    let mut items_height = use_signal(50);

    let items = use_signal(vec![
        Edge::new()
            .width(Length::Fixed(20))
            .height::<u32>(items_height)
            .with_style(BoxStyle::base().background_color(Rgb888::new(
                random(),
                random(),
                random(),
            )))
            .el(),
        Space::row(Length::Fixed(100)).el(),
        Edge::new()
            .width(Length::Fixed(20))
            .height::<u32>(items_height)
            .with_style(BoxStyle::base().background_color(Rgb888::new(
                random(),
                random(),
                random(),
            )))
            .el(),
    ]);

    // let items: [_; 5] = array::from_fn(|_| {
    //     Edge::new()
    //         .width(Length::Div(5))
    //         .height::<u32>(items_height)
    //         .with_style(BoxStyle::base().background_color(Rgb888::new(
    //             random(),
    //             random(),
    //             random(),
    //         )))
    //         .el()
    // });

    let flexbox = Flex::row(items)
        .wrap(true)
        .horizontal_align(Align::Center)
        .width(Length::fill());

    let mut ui = UI::new(flexbox.el(), display.bounding_box().size)
        .on_exit(|| std::process::exit(1));

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

        window.events().for_each(|e| {});
        ui.tick([NullEvent].into_iter());
        ui.draw(&mut display).unwrap();

        window.update(&display);
    }
}

use cap::Cap;
use embedded_graphics::{ pixelcolor::BinaryColor, prelude::{ Dimensions, Point } };
use embedded_graphics_simulator::{ OutputSettingsBuilder, SimulatorDisplay, Window };
use rand::{ Rng, rng, thread_rng };
use rsact_icons::{ common::CommonIcon, system::SystemIcon };
use rsact_reactive::runtime::current_runtime_profile;
use rsact_ui::{
    event::{ message::UiQueue, simulator::simulator_single_encoder },
    layout::{ Align, size::{ PointExt, Size, UnitV2 } },
    prelude::{
        Button,
        Icon,
        IntoInert,
        ReadSignal,
        Scrollable,
        SignalMap,
        Text,
        UiMessage,
        WriteSignal,
        create_effect,
        create_signal,
        with_current_runtime,
    },
    style::NullStyler,
    ui::UI,
    utils::lerpi,
    value::RangeU8,
    widget::{ BlockModelWidget, SizedWidget, Widget, bar::Bar, flex::Flex, knob::Knob },
};
use std::{
    alloc::System,
    format,
    println,
    string::{ String, ToString },
    time::{ Duration, Instant },
    vec::Vec,
};

#[global_allocator]
static GLOBAL: Cap<System> = Cap::new(System, usize::MAX);

fn main() {
    let output_settings = OutputSettingsBuilder::new()
        .max_fps(10000)
        .scale(5)
        // .theme(embedded_graphics_simulator::BinaryColorTheme::OledWhite)
        .build();

    let mut window = Window::new("3D Printers", &output_settings);

    let mut display = SimulatorDisplay::<BinaryColor>::new(Size::new(128, 64).into());

    window.update(&display);

    let mem_init = GLOBAL.allocated();

    let queue = UiQueue::new();
    let main_page_id = "main";

    let back_button = || {
        Button::new(
            Flex::row([Icon::new(SystemIcon::ArrowLeft).size(6u32).el(), "Back".into()])
                .gap(2u32)
                .center()
        )
            .padding(2u32)
            .on_click(move || {
                queue.publish(UiMessage::GoTo(main_page_id));
            })
            .el()
    };

    // This is not a good way to implement animations/logic, this's just to simulate printing process
    let mut printing_file = create_signal(String::new());
    let mut printing_progress_anim_ts = create_signal(Instant::now());
    let mut is_printing = create_signal(false);
    let mut printing_progress = create_signal(RangeU8::new_full_range(0));
    let print_page_id = "print";
    let print_page = Flex::col([
        Bar::horizontal(printing_progress).el(),
        Text::new(printing_file.map(|filename| format!("Printing {filename}..."))).el(),
    ])
        .fill()
        .gap(10u32)
        .padding(10u32)
        .horizontal_align(Align::Center)
        .el();

    create_effect(move |_| {
        if is_printing.get() {
            if printing_progress.get().is_max() {
                is_printing.set(false);
                queue.publish(UiMessage::PreviousPage);
            } else if printing_progress_anim_ts.get().elapsed().as_millis() > 10 {
                printing_progress.set(printing_progress.get() + rng().random_range(0..5));
                printing_progress_anim_ts.set(Instant::now());
            }
        }
    });

    let mut print_file = move |filename: &str| {
        printing_file.set(filename.to_string());
        printing_progress.set((0).into());
        printing_progress_anim_ts.set(Instant::now());

        queue.goto(print_page_id);

        is_printing.set(true);
    };

    let files_page_id = "files";
    let files = fake::vec![String as fake::faker::lorem::en::Word(); 10..15];
    let files_page = Scrollable::vertical(
        Flex::col(
            core::iter
                ::once(back_button())
                .chain(
                    files.into_iter().map(|filename| {
                        Button::new(filename.as_str())
                            .fill_width()
                            .on_click(move || {
                                print_file(&filename);
                            })
                            .el()
                    })
                )
                .collect::<Vec<_>>()
        )
            .fill_width()
            .gap(1u32)
            .el()
    )
        .tracker()
        .fill()
        .el();

    const MAX_POSITION: Point = Point::new(250, 200);
    let max_z = 200;
    let mut position = create_signal(Point::new(2, 87));
    let mut z_pos = create_signal(35i32);
    let position_page_id = "position";
    let move_distance = 1;
    let position_button = |text: &str, dir: UnitV2| {
        Button::new(text)
            .on_click(move || {
                position.update(move |pos| {
                    *pos = (*pos + dir * move_distance).clamp_axes(Point::zero(), MAX_POSITION);
                })
            })
            .padding(3u32)
            .el()
    };

    // Duration in milliseconds
    let parking_home_anim_dur = 10000;
    let mut parking_home = create_signal(false);
    let mut home_anim_ts = create_signal(Instant::now());
    create_effect(move |_| {
        if parking_home.get() {
            if z_pos.get() == 0 && position.get() == Point::zero() {
                parking_home.set(false);
            } else if let elapsed @ 1.. = home_anim_ts.get().elapsed().as_millis() {
                position.update(move |pos| {
                    pos.x = lerpi(pos.x, 0, elapsed as i32, parking_home_anim_dur);
                    pos.y = lerpi(pos.x, 0, elapsed as i32, parking_home_anim_dur);
                });
                z_pos.update(move |z_pos| {
                    *z_pos = lerpi(*z_pos, 0, elapsed as i32, parking_home_anim_dur);
                });
                home_anim_ts.set(Instant::now());
            }
        }
    });

    let position_page = Flex::row([
        Flex::col([
            back_button(),
            Text::new(z_pos.map(|z_pos| format!("{z_pos}Z"))).el(),
            Button::new("Home")
                .padding(2u32)
                .on_click(move || {
                    home_anim_ts.set(Instant::now());
                    parking_home.set(true);
                })
                .el(),
        ])
            .padding(1u32)
            .fill_height()
            .center()
            .gap(3u32)
            .el(),
        Flex::col([
            Button::new("Z+")
                .on_click(move || {
                    z_pos.update(|z_pos| {
                        *z_pos = (*z_pos + 1).min(max_z);
                    });
                })
                .padding(2u32)
                .el(),
            position_button("X-", UnitV2::LEFT),
            Button::new("Z-")
                .on_click(move || {
                    z_pos.update(|z_pos| {
                        *z_pos = (*z_pos - 1).max(0);
                    });
                })
                .padding(2u32)
                .el(),
        ])
            .center()
            .gap(1u32)
            .fill_height()
            .el(),
        Flex::col([
            position_button("Y-", UnitV2::UP),
            Text::new(position.map(|pos| pos.to_string())).el(),
            position_button("Y+", UnitV2::DOWN),
        ])
            .gap(5u32)
            .center()
            .fill()
            .el(),
        Flex::col([position_button("X+", UnitV2::RIGHT)])
            .center()
            .fill_height()
            .el(),
    ])
        .fill()
        .el();

    let temp_page_id = "temp";
    let mut bed_temp = create_signal(RangeU8::<0, 110>::new_clamped(60));
    // I know that nozzle temperature can be bigger than 255, but that's just a simulation
    let mut nozzle_temp = create_signal(RangeU8::<0, 250>::new_clamped(220));

    let mut cool_anim_ts = create_signal(Instant::now());
    let cool_anim_dur = 10000;
    let mut cooling = create_signal(false);
    create_effect(move |_| {
        if cooling.get() {
            if bed_temp.get() <= 25 && nozzle_temp.get() <= 25 {
                cooling.set(false);
            } else if let elapsed @ 1.. = cool_anim_ts.get().elapsed().as_millis() {
                bed_temp.update(|temp| {
                    temp.set(lerpi(temp.inner() as u32, 20, elapsed as u32, cool_anim_dur) as u8);
                });
                nozzle_temp.update(|temp| {
                    temp.set(lerpi(temp.inner() as u32, 20, elapsed as u32, cool_anim_dur) as u8);
                });
            }
        }
    });

    // TODO: Nozzle and bed icons?
    let temp_page = Flex::row([
        Flex::col([
            back_button(),
            Button::new("Cool")
                .on_click(move || {
                    cool_anim_ts.set(Instant::now());
                    cooling.set(true);
                })
                .padding(3u32)
                .el(),
        ])
            .center()
            .gap(5u32)
            .fill()
            .el(),
        Flex::col([
            Text::new(bed_temp.map(|temp| format!("{temp}C"))).el(),
            Knob::new(bed_temp).el(),
            Text::new_inert("Bed").el(),
        ])
            .gap(2u32)
            .center()
            .fill()
            .el(),
        Flex::col([
            Text::new(nozzle_temp.map(|temp| format!("{temp}C"))).el(),
            Knob::new(nozzle_temp).el(),
            Text::new_inert("Nozzle").el(),
        ])
            .gap(2u32)
            .center()
            .fill()
            .el(),
    ])
        .fill()
        .el();

    let main = Scrollable::vertical(
        Flex::col([
            Button::new(
                Flex::row([Icon::new(CommonIcon::File).size(8u32).el(), "Files".into()]).gap(3u32)
            )
                .on_click(move || {
                    queue.publish(UiMessage::GoTo(files_page_id));
                })
                .fill_width()
                .el(),
            Button::new(
                Flex::row([
                    Icon::new(CommonIcon::MapMarker).size(8u32).el(),
                    "Position".into(),
                ]).gap(3u32)
            )
                .on_click(move || {
                    queue.publish(UiMessage::GoTo(position_page_id));
                })
                .fill_width()
                .el(),
            Button::new(
                Flex::row([
                    Icon::new(CommonIcon::Thermometer).size(8u32).el(),
                    "Temperature".into(),
                ]).gap(3u32)
            )
                .on_click(move || {
                    queue.publish(UiMessage::GoTo(temp_page_id));
                })
                .fill_width()
                .el(),
        ])
            .gap(1u32)
            .fill_width()
            .el()
    )
        .tracker()
        .fill()
        .el();

    let mut ui = UI::new_with_buffer_renderer(
        display.bounding_box().size.inert(),
        NullStyler,
        BinaryColor::Off
        // AccentStyler::new(Rgb888::RED),
    )
        // .with_renderer_options(
        //     LayeringRendererOptions::new().anti_aliasing(AntiAliasing::Enabled),
        // )
        .auto_focus()
        .on_exit(|| std::process::exit(0))
        .with_page(main_page_id, main)
        .with_page(print_page_id, print_page)
        .with_page(position_page_id, position_page)
        .with_page(temp_page_id, temp_page)
        .with_page(files_page_id, files_page)
        .with_queue(queue);

    println!(
        "Initialization mem use: {:0.3}KiB",
        ((GLOBAL.allocated() - mem_init) as f32) / 1024.0
    );

    // std::fs::write(
    //     "./graph.mmd",
    //     format!(
    //         // "```mermaid\n{}\n```",
    //         "{}",
    //         with_current_runtime(|rt| rt.global_mermaid_graph(1_000_000))
    //     ),
    // )
    // .unwrap();

    let mut fps = 0;
    let mut last_time = Instant::now();
    let mut mem_leaked = 0;
    let mut total_mem_leaked = 0;
    loop {
        let now = Instant::now();
        if now - last_time >= Duration::from_secs(1) {
            println!("{fps}FPS");
            println!("{} draw calls", ui.current_page().take_draw_calls());

            // let stats = rsact_ui_region.change();
            // println!("Heap", stats.bytes_allocated - stats.by);

            // println!("Heap allocated: {:0.3}KiB", used_mem as f32 / 1024.0);
            // println!(
            //     "mem usage: {:0.3}KiB",
            //     GLOBAL.allocated() as f32 / 1024.0
            // );

            // Leaked mem here mostly means for me that new data was allocated in reactive runtime, not actual "bad" leaks :)
            total_mem_leaked += mem_leaked;

            println!(
                "Mem leaked: {:0.3}KiB (total of {:0.3}KiB)",
                (mem_leaked as f32) / 1024.0,
                (total_mem_leaked as f32) / 1024.0
            );
            mem_leaked = 0;

            println!("Reactive runtime profile: {}", current_runtime_profile());

            fps = 0;
            last_time = now;
        } else {
            fps += 1;
            printing_progress_anim_ts.notify();
            home_anim_ts.notify();
            cool_anim_ts.notify();
        }

        let mem_start = GLOBAL.allocated();
        let events = window
            .events()
            .map(simulator_single_encoder)
            .filter_map(|e| e)
            .collect::<Vec<_>>();

        // let _scope = new_scope();
        let _unhandled = ui
            .tick(events.into_iter().inspect(|e| println!("Event: {e:?}")))
            .iter()
            .inspect(|unhandled| {
                println!("Unhandled event {unhandled:?}");
            });

        ui.render(&mut display);
        mem_leaked += GLOBAL.allocated().saturating_sub(mem_start);

        window.update(&display);
    }
}

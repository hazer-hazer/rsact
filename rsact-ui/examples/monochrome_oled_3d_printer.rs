use cap::Cap;
use embedded_graphics::{
    pixelcolor::{BinaryColor, Rgb888},
    prelude::{Dimensions, Point, RgbColor},
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use fake::{faker, locales::EN, Fake};
use rand::{random, thread_rng, Rng};
use rsact_reactive::runtime::{current_runtime_profile, new_scope};
use rsact_ui::{
    event::{message::MessageQueue, simulator::simulator_single_encoder},
    layout::{
        size::{PointExt, Size, UnitV1, UnitV2},
        Align,
    },
    prelude::{
        create_memo, create_signal, use_effect, AsMemo, Button, Container,
        Length, Message, MonoText, ReadSignal, Scrollable, SignalMapper,
        WriteSignal,
    },
    render::draw_target::{AntiAliasing, LayeringRendererOptions},
    style::{accent::AccentStyler, NullStyler},
    ui::UI,
    utils::lerpi,
    value::RangeU8,
    widget::{
        bar::Bar, flex::Flex, knob::Knob, BlockModelWidget, SizedWidget,
        Widget as _,
    },
};
use std::{
    alloc::System,
    fmt::Display,
    format, println,
    string::{String, ToString},
    time::{Duration, Instant},
    vec::Vec,
};

#[global_allocator]
static GLOBAL: Cap<System> = Cap::new(System, usize::MAX);

// struct MemoryMeter {
//     // on_start: usize,
//     total: usize,
// }

// impl Display for MemoryMeter {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         // let used = self.on_start.saturating_sub(self.total);
//         match self.total {
//             0..1024 => write!(f, "{}B", self.total),
//             _ => write!(f, "{:0.3}KiB", (self.total as f32) / 1024.0),
//         }
//     }
// }

// impl MemoryMeter {
//     fn new() -> Self {
//         Self { total: 0 }
//     }

//     fn start(self) -> MemoryMeasure {
//         MemoryMeasure::new(self)
//     }
// }

// struct MemoryMeasure {
//     on_start: usize,
//     meter: MemoryMeter,
// }

// impl MemoryMeasure {
//     fn new(meter: MemoryMeter) -> Self {
//         Self { on_start: GLOBAL.allocated(), meter }
//     }

//     fn end(mut self) -> MemoryMeter {
//         self.meter.total += GLOBAL.allocated();
//         self.meter.total -= self.on_start;
//         self.meter
//     }
// }

/**
 * This example simulates imaginable 3D printer UI on monochrome 128x64 OLED display.
 */

fn main() {
    let output_settings = OutputSettingsBuilder::new()
        .max_fps(10000)
        // .scale(5)
        .theme(embedded_graphics_simulator::BinaryColorTheme::OledWhite)
        .build();

    let mut window = Window::new("SANDBOX", &output_settings);

    let mut display =
        SimulatorDisplay::<BinaryColor>::new(Size::new(128, 64).into());

    window.update(&display);

    let mem_init = GLOBAL.allocated();

    let queue = MessageQueue::new();
    let main_page_id = "main";

    let back_button = || {
        Button::new("Back")
            .padding(2)
            .on_click(move || {
                queue.publish(Message::GoTo(main_page_id));
            })
            .el()
    };

    // This is not a good way to implement animations/logic, this's just to simulate printing process
    let printing_file = create_signal(String::new());
    let printing_progress_anim_ts = create_signal(Instant::now());
    let is_printing = create_signal(false);
    let printing_progress = create_signal(RangeU8::new_full_range(0));
    let print_page_id = "print";
    let print_page = Flex::col([
        Bar::horizontal(printing_progress).el(),
        MonoText::new(
            printing_file.mapped(|filename| format!("Printing {filename}...")),
        )
        .el(),
    ])
    .fill()
    .gap(10)
    .padding(10)
    .horizontal_align(Align::Center)
    .el();

    use_effect(move |_| {
        if is_printing.get() {
            if printing_progress.get().is_max() {
                is_printing.set(false);
                queue.publish(Message::PreviousPage);
            } else if printing_progress_anim_ts.get().elapsed().as_millis() > 10
            {
                printing_progress.set(
                    printing_progress.get() + thread_rng().gen_range(0..5),
                );
                printing_progress_anim_ts.set(Instant::now());
            }
        }
    });

    let print_file = move |filename: &str| {
        printing_file.set(filename.to_string());
        printing_progress.set(0.into());
        printing_progress_anim_ts.set(Instant::now());

        queue.publish(Message::GoTo(print_page_id));

        is_printing.set(true);
    };

    let files_page_id = "files";
    let files = fake::vec![String as fake::faker::lorem::en::Word(); 10..15];
    let files_page = Scrollable::vertical(
        Flex::col(
            core::iter::once(back_button())
                .chain(files.into_iter().map(|filename| {
                    Button::new(filename.as_str())
                        .fill_width()
                        .on_click(move || {
                            print_file(&filename);
                        })
                        .el()
                }))
                .collect::<Vec<_>>(),
        )
        .fill_width()
        .gap(1)
        .el(),
    )
    .tracker()
    .fill()
    .el();

    const MAX_POSITION: Point = Point::new(250, 200);
    let max_z = 200;
    let position = create_signal(Point::new(2, 87));
    let z_pos = create_signal(35i32);
    let position_page_id = "position";
    let move_distance = 1;
    let position_button = |text: &str, dir: UnitV2| {
        Button::new(text)
            .on_click(move || {
                position.update(move |pos| {
                    *pos = (*pos + (dir * move_distance))
                        .clamp_axes(Point::zero(), MAX_POSITION);
                })
            })
            .padding(3)
            .el()
    };

    // Duration in milliseconds
    let parking_home_anim_dur = 10000;
    let parking_home = create_signal(false);
    let home_anim_ts = create_signal(Instant::now());
    use_effect(move |_| {
        if parking_home.get() {
            if z_pos.get() == 0 && position.get() == Point::zero() {
                parking_home.set(false);
            } else if let elapsed @ 1.. =
                home_anim_ts.get().elapsed().as_millis()
            {
                position.update(move |pos| {
                    pos.x =
                        lerpi(pos.x, 0, elapsed as i32, parking_home_anim_dur);
                    pos.y =
                        lerpi(pos.x, 0, elapsed as i32, parking_home_anim_dur);
                });
                z_pos.update(move |z_pos| {
                    *z_pos =
                        lerpi(*z_pos, 0, elapsed as i32, parking_home_anim_dur);
                });
                home_anim_ts.set(Instant::now());
            }
        }
    });

    let position_page = Flex::row([
        Flex::col([
            back_button(),
            MonoText::new(z_pos.mapped(|z_pos| format!("{z_pos}Z"))).el(),
            Button::new("Home")
                .padding(2)
                .on_click(move || {
                    home_anim_ts.set(Instant::now());
                    parking_home.set(true);
                })
                .el(),
        ])
        .fill()
        .center()
        .gap(3)
        .el(),
        Flex::col([
            Button::new("Z+")
                .on_click(move || {
                    z_pos.update(|z_pos| *z_pos = (*z_pos + 1).min(max_z));
                })
                .padding(2)
                .el(),
            position_button("X-", UnitV2::LEFT),
            Button::new("Z-")
                .on_click(move || {
                    z_pos.update(|z_pos| *z_pos = (*z_pos - 1).max(0));
                })
                .padding(2)
                .el(),
        ])
        .center()
        .gap(1)
        .fill()
        .el(),
        Flex::col([
            position_button("Y-", UnitV2::UP),
            MonoText::new(position.as_memo()).el(),
            position_button("Y+", UnitV2::DOWN),
        ])
        .gap(5)
        .center()
        .fill()
        .el(),
        Flex::col([position_button("X+", UnitV2::RIGHT)]).center().fill().el(),
    ])
    .fill()
    .el();

    let temp_page_id = "temp";
    let bed_temp = create_signal(RangeU8::<0, 110>::new_clamped(60));
    // I know that nozzle temperature can be bigger than 255, but that's just a simulation
    let nozzle_temp = create_signal(RangeU8::<0, 250>::new_clamped(220));

    let cool_anim_ts = create_signal(Instant::now());
    let cool_anim_dur = 10000;
    let cooling = create_signal(false);
    use_effect(move |_| {
        if cooling.get() {
            if bed_temp.get() <= 25 && nozzle_temp.get() <= 25 {
                cooling.set(false);
            } else if let elapsed @ 1.. =
                cool_anim_ts.get().elapsed().as_millis()
            {
                bed_temp.update(|temp| {
                    temp.set(lerpi(
                        temp.inner() as u32,
                        20,
                        elapsed as u32,
                        cool_anim_dur,
                    ) as u8);
                });
                nozzle_temp.update(|temp| {
                    temp.set(lerpi(
                        temp.inner() as u32,
                        20,
                        elapsed as u32,
                        cool_anim_dur,
                    ) as u8);
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
                .padding(3)
                .el(),
        ])
        .center()
        .gap(5)
        .fill()
        .el(),
        Flex::col([
            MonoText::new(bed_temp.mapped(|temp| format!("{temp}C"))).el(),
            Knob::new(bed_temp).el(),
            MonoText::new_static("Bed").el(),
        ])
        .gap(2)
        .center()
        .fill()
        .el(),
        Flex::col([
            MonoText::new(nozzle_temp.mapped(|temp| format!("{temp}C"))).el(),
            Knob::new(nozzle_temp).el(),
            MonoText::new_static("Nozzle").el(),
        ])
        .gap(2)
        .center()
        .fill()
        .el(),
    ])
    .fill()
    .el();

    let main = Scrollable::vertical(
        Flex::col([
            Button::new("Files")
                .on_click(move || {
                    queue.publish(Message::GoTo(files_page_id));
                })
                .fill_width()
                .el(),
            Button::new("Position")
                .on_click(move || {
                    queue.publish(Message::GoTo(position_page_id));
                })
                .fill_width()
                .el(),
            Button::new("Temperature")
                .on_click(move || {
                    queue.publish(Message::GoTo(temp_page_id));
                })
                .fill_width()
                .el(),
        ])
        .gap(1)
        .fill_width()
        .el(),
    )
    .tracker()
    .fill()
    .el();

    let mut ui = UI::new(
        main_page_id,
        main,
        display.bounding_box().size,
        NullStyler
        // AccentStyler::new(Rgb888::RED),
    )
    // .with_renderer_options(
    //     LayeringRendererOptions::new().anti_aliasing(AntiAliasing::Enabled),
    // )
    .on_exit(|| std::process::exit(0))
    .with_page(print_page_id, print_page)
    .with_page(position_page_id, position_page)
    .with_page(temp_page_id, temp_page)
    .with_page(files_page_id, files_page)
    .with_queue(queue);

    ui.current_page().auto_focus();

    // let mem_init = mem_init.end();
    println!("Initialization mem use: {}", GLOBAL.allocated() - mem_init);

    let mut fps = 0;
    let mut last_time = Instant::now();
    let mut mem_leaked = 0;
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
            println!("Mem leaked: {:0.3}KiB", mem_leaked as f32 / 1024.0);
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

        ui.draw(&mut display);
        mem_leaked += GLOBAL.allocated().saturating_sub(mem_start);

        window.update(&display);
    }
}

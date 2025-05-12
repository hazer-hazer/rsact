use embedded_graphics::{
    mono_font::ascii::FONT_4X6,
    pixelcolor::Rgb565,
    prelude::{Dimensions, RgbColor},
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use paw::{
    modx::{lfo::LfoWaveform, mod_pack::ModTarget},
    param::f32::UnitInterval,
};
use rsact_ui::{
    col,
    event::simulator::simulator_single_encoder,
    font::FontImport,
    layout::size::Size,
    page::id::SinglePage,
    prelude::{Checkbox, IntoInert, Select, Slider, create_signal},
    render::{AntiAliasing, RendererOptions},
    row,
    style::NullStyler,
    ui::UI,
    widget::{SizedWidget, Widget, flex::Flex},
};
use std::time::{Duration, Instant};

fn main() {
    let output_settings =
        OutputSettingsBuilder::new().max_fps(10000).scale(5).build();

    let mut window = Window::new("SANDBOX", &output_settings);

    let mut display =
        SimulatorDisplay::<Rgb565>::new(Size::new(160, 80).into());

    window.update(&display);

    let wt_depth = create_signal(0.0);
    let lfo_enabled = create_signal(false);
    let lfo_waveform = create_signal(LfoWaveform::default());
    let lfo_target = create_signal(ModTarget::default());

    let page = row![
        col![Slider::vertical(wt_depth, (0.0..=3.0).inert())].fill(),
        col![
            Checkbox::new(lfo_enabled),
            Select::horizontal(
                lfo_target,
                ModTarget::each::<1>().collect::<Vec<_>>().inert()
            ),
            Select::horizontal(
                lfo_waveform,
                LfoWaveform::each(UnitInterval::EQUILIBRIUM)
                    .collect::<Vec<_>>()
                    .inert()
            ),
        ]
        .fill()
    ]
    .center()
    .fill();

    let mut ui = UI::new_with_buffer_renderer(
        display.bounding_box().size.inert(),
        NullStyler,
        Rgb565::WHITE,
    )
    .auto_focus()
    .with_default_font(FontImport::fixed_eg_mono_font(&FONT_4X6))
    .with_page(SinglePage, page.el())
    .with_renderer_options(
        RendererOptions::new().anti_aliasing(AntiAliasing::Enabled),
    )
    .on_exit(|| std::process::exit(0));

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

        ui.tick(
            window
                .events()
                .map(simulator_single_encoder)
                .filter_map(|e| e)
                .inspect(|e| println!("Event: {e:?}")),
        );

        ui.render(&mut display);
        window.update(&display);
    }
}

use criterion::{Criterion, criterion_group, criterion_main};
use embedded_graphics::{pixelcolor::Rgb888, prelude::RgbColor};
use rsact_reactive::{
    effect::create_effect, prelude::*, runtime::with_new_runtime,
};
use rsact_ui::{
    page::id::SinglePage,
    prelude::{Edge, Flex, Size},
    render::NullDrawTarget,
    style::NullStyler,
    ui::UI,
    widget::{SizedWidget, Widget},
};

fn bench(c: &mut Criterion) {
    const EDGES_COUNT: usize = 10_000;

    let mut ui = UI::new_with_buffer_renderer(
        Size::new(100, 100).inert(),
        NullStyler,
        Rgb888::BLACK,
    )
    .with_page(
        SinglePage,
        Flex::row(core::array::from_fn::<_, EDGES_COUNT, _>(|_| {
            Edge::new().width(100u32).height(100u32).el()
        })),
    )
    .no_events();

    c.bench_function(&format!("Render {EDGES_COUNT} edges"), |b| {
        b.iter(|| {
            ui.current_page().force_redraw();
            ui.render(&mut NullDrawTarget::default())
        })
    });
}

criterion_group! {
    name = reactivity;
    config = Criterion::default();
    targets = bench
}
criterion_main!(reactivity);

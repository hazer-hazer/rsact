//! UI size probe: a headless 10-label page built + laid out + rendered through
//! the public UI API with a NullRenderer, so `.text` reflects the widget/layout/
//! render footprint of a small page. Never run (the null theme may panic at
//! render time); it only has to link — that's all a size measurement needs.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::{format, string::String, vec::Vec};
use core::hint::black_box;
use cortex_m_rt::entry;
use rsact_ui::{el::ctx::Wtf, prelude::*, ui::UI};

type NullWtf = Wtf<NullRenderer, (), (), ()>;

#[entry]
fn main() -> ! {
    size_probe::init_heap();

    let labels: Vec<Signal<String>> = (0..10)
        .map(|i| create_signal(format!("label {i}")))
        .collect();
    let init = labels.clone();

    let mut renderer = NullRenderer::default();
    let mut ui: UI<NullWtf, _> =
        UI::new((), Size::zero()).with_page((), move || {
            Flex::col(
                init.iter()
                    .map(|s| Label::new(*s).into_el())
                    .collect::<Vec<_>>(),
            )
            .into_el()
        });
    let _ = ui.current_page();
    // The one render path (WS6.4d): a frame is `start_frame` plus its region
    // loop. The size probe measures what the real path monomorphizes to.
    let mut frame = ui.start_frame(&mut renderer);
    while frame.render(&mut renderer).is_some() {}
    drop(frame);

    black_box(&ui);
    loop {}
}

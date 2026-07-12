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

    let mut ui: UI<NullWtf, _> =
        UI::new((), NullRenderer).with_page((), move || {
            Flex::col(
                init.iter()
                    .map(|s| Label::new(*s).into_el())
                    .collect::<Vec<_>>(),
            )
            .into_el()
        });
    let _ = ui.current_page();
    ui.current_page().use_renderer(|_| {});

    black_box(&ui);
    loop {}
}

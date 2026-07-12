//! Shared, `#[doc(hidden)]` test/bench scaffolding (WS0.7j). Both the
//! `metrics-probe` tool and the `layout` bench built the same headless N-label
//! page independently; a single builder here keeps them from drifting so their
//! numbers stay comparable. Not part of the public API.

use crate::{
    el::ctx::Wtf,
    prelude::*,
    ui::{UI, WithPages},
};
use alloc::{format, string::String, vec::Vec};

/// The headless widget context: no-op renderer, unit page-id / stylist / event.
pub type NullWtf = Wtf<NullRenderer, (), (), ()>;

/// Build a headless page of `n` labels, each bound to a signal, and return the
/// built `UI` plus the label signals (so a caller can dirty one to force a
/// relayout). The active page's arena + layout tree are built before returning.
pub fn labels_page(n: usize) -> (UI<NullWtf, WithPages>, Vec<Signal<String>>) {
    let labels: Vec<Signal<String>> = (0..n)
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
    (ui, labels)
}

/// Build a headless page of `n` buttons in a column, each wrapping a label
/// bound to a signal (button-heavy: `n` `ButtonBuilder -> Button` transforms,
/// WS13.2 Task 5). Returns the built `UI` plus the label signals, so a caller
/// can dirty one to force a relayout.
pub fn buttons_page(n: usize) -> (UI<NullWtf, WithPages>, Vec<Signal<String>>) {
    let labels: Vec<Signal<String>> = (0..n)
        .map(|i| create_signal(format!("button {i}")))
        .collect();
    let init = labels.clone();
    let mut ui: UI<NullWtf, _> =
        UI::new((), NullRenderer).with_page((), move || {
            Flex::col(
                init.iter()
                    .map(|s| Button::new(Label::new(*s)).into_el())
                    .collect::<Vec<_>>(),
            )
            .into_el()
        });
    let _ = ui.current_page();
    (ui, labels)
}

/// Build a headless page of `n` nested `Flex` containers (flex-heavy: `n + 1`
/// `FlexBuilder -> Flex` transforms — one outer column plus `n` inner rows,
/// each wrapping one label bound to a signal, WS13.2 Task 5). Returns the
/// built `UI` plus the label signals, so a caller can dirty one to force a
/// relayout.
pub fn nested_flex_page(
    n: usize,
) -> (UI<NullWtf, WithPages>, Vec<Signal<String>>) {
    let labels: Vec<Signal<String>> = (0..n)
        .map(|i| create_signal(format!("nested {i}")))
        .collect();
    let init = labels.clone();
    let mut ui: UI<NullWtf, _> =
        UI::new((), NullRenderer).with_page((), move || {
            Flex::col(
                init.iter()
                    .map(|s| Flex::row([Label::new(*s).into_el()]).into_el())
                    .collect::<Vec<_>>(),
            )
            .into_el()
        });
    let _ = ui.current_page();
    (ui, labels)
}

use rsact_reactive::maybe::maybe_signal::MaybeSignal;

use crate::el::{El, WidgetCtx};

/// Anything that can act as a piece of UI, i.e. be turned into an [`El`].
///
/// This is the conversion accepted everywhere a single child / UI piece is
/// expected (container content, page roots, `row!`/`col!` children, ...).
///
/// - Every widget implements it with a one-line `impl View` next to its
///   `Widget` impl — it just wraps itself into an `El` via `Widget::el`.
/// - Leaf values build an appropriate widget first (e.g. `&str`/`String` build
///   a [`Label`](crate::widget::label::Label)).
/// - Containers (`Option<V>`, factory closures, ...) compose other `View`s.
///
/// Note: there is intentionally **no** blanket `impl<T: Widget<W>> View<W> for
/// T`. Such a blanket collides (Rust coherence does no negative reasoning) with
/// concrete leaf impls like `View for &str`, because the compiler cannot prove
/// `&str: !Widget<W>`. Instead each widget gets its own concrete impl, which
/// never overlaps with the leaf impls.
pub trait View<W: WidgetCtx> {
    fn into_el(self) -> El<W>;
}

impl<W: WidgetCtx> View<W> for El<W> {
    fn into_el(self) -> El<W> {
        self
    }
}

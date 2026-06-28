use rsact_reactive::maybe::maybe_signal::MaybeSignal;
use rsact_reactive::prelude::Signal;

use crate::el::{El, WidgetCtx};
use alloc::vec::Vec;

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

/// A sequence of [`View`]s usable as the children of a multi-child widget
/// (currently [`Flex`](crate::widget::flex::Flex)).
///
/// Unlike accepting a ready-made `Vec<El>`, this auto-erases each element via
/// [`View::into_el`], so heterogeneous inputs work directly without manual
/// `.el()` calls:
/// - a **tuple** of different widget types, e.g. `(Button, "label", Checkbox)`
/// - a homogeneous **array** `[V; N]` (incl. `[El; N]`, since `El: View`)
/// - a **`Vec<V>`** of views (incl. `Vec<El>`)
/// - a reactive **`Signal<Vec<El>>`** (already erased; reactivity preserved)
///
/// Static inputs are stored inert ([`MaybeSignal::new_inert`]) — no per-widget
/// signal node is created; only the `Signal` input keeps a reactive children
/// list.
pub trait ViewSequence<W: WidgetCtx> {
    fn into_children(self) -> MaybeSignal<Vec<El<W>>>;
}

impl<W: WidgetCtx + 'static, V: View<W>, const N: usize> ViewSequence<W>
    for [V; N]
{
    fn into_children(self) -> MaybeSignal<Vec<El<W>>> {
        MaybeSignal::new_inert(self.into_iter().map(|v| v.into_el()).collect())
    }
}

impl<W: WidgetCtx + 'static, V: View<W>> ViewSequence<W> for Vec<V> {
    fn into_children(self) -> MaybeSignal<Vec<El<W>>> {
        MaybeSignal::new_inert(self.into_iter().map(|v| v.into_el()).collect())
    }
}

impl<W: WidgetCtx + 'static> ViewSequence<W> for Signal<Vec<El<W>>> {
    fn into_children(self) -> MaybeSignal<Vec<El<W>>> {
        self.into()
    }
}

/// Tuples of heterogeneous views become children, each erased via
/// [`View::into_el`]. Implemented for arities 1..=12.
macro_rules! impl_view_sequence_tuple {
    ($($T:ident),+) => {
        impl<W: WidgetCtx + 'static, $($T: View<W>),+> ViewSequence<W>
            for ($($T,)+)
        {
            fn into_children(self) -> MaybeSignal<Vec<El<W>>> {
                #[allow(non_snake_case)]
                let ($($T,)+) = self;
                MaybeSignal::new_inert(alloc::vec![$($T.into_el()),+])
            }
        }
    };
}

impl_view_sequence_tuple!(A);
impl_view_sequence_tuple!(A, B);
impl_view_sequence_tuple!(A, B, C);
impl_view_sequence_tuple!(A, B, C, D);
impl_view_sequence_tuple!(A, B, C, D, E);
impl_view_sequence_tuple!(A, B, C, D, E, F);
impl_view_sequence_tuple!(A, B, C, D, E, F, G);
impl_view_sequence_tuple!(A, B, C, D, E, F, G, H);
impl_view_sequence_tuple!(A, B, C, D, E, F, G, H, I);
impl_view_sequence_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_view_sequence_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_view_sequence_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);

use crate::{
    maybe::{maybe_reactive::MaybeReactive, maybe_signal::MaybeSignal},
    memo::{IntoMemo as _, Memo, create_memo},
    read::SignalMap as _,
};

// pub mod inert;
pub mod maybe_reactive;
pub mod maybe_signal;

// TODO: This is kinda shitty trait, remove it, it's like "make a memo from anything even if it's not reactive", but if it's not reactive then the memo is just a wrapper around inert value and won't update.
/// A [`SignalMap`] variant that **always** returns a [`Memo<U>`].
///
/// Unlike [`SignalMap::map`], which preserves the reactivity of the source
/// (inert in → inert out, reactive in → reactive out), `map_reactive`
/// unconditionally produces a tracked memo node:
///
/// - For a reactive source ([`MaybeSignal::Signal`], [`MaybeReactive::Memo`],
///   etc.) the returned memo re-evaluates `map` whenever the source changes.
/// - For a [`MaybeSignal::Inert`] source the returned memo is a **constant**:
///   the closure is evaluated once at call time and the memo never
///   re-evaluates. If the `MaybeSignal` is later promoted to a `Signal` via
///   [`WriteSignal`], the already-returned `Memo<U>` is **not** updated.
///
/// # Example (real usage in `rsact-ui`)
///
/// ```rust,ignore
/// // Slider: range may be static or reactive, but step is always a live Memo.
/// let step = range.map_reactive(|r| Self::step_from_range(r));
///
/// // Flex: children list may be static or reactive,
/// // but layout children must always be a live Memo.
/// let layout_children = children.map_reactive(|children| {
///     children.iter().map(|c| c.layout().memo()).collect()
/// });
/// ```
pub trait SignalMapReactive<T> {
    fn map_reactive<U: PartialEq + Clone + 'static>(
        &self,
        map: impl Fn(&T) -> U + 'static,
    ) -> Memo<U>;
}

impl<T: 'static> SignalMapReactive<T> for MaybeSignal<T> {
    #[track_caller]
    fn map_reactive<U: PartialEq + Clone + 'static>(
        &self,
        map: impl Fn(&T) -> U + 'static,
    ) -> Memo<U> {
        match self {
            MaybeSignal::Inert(inert) => {
                let mapped = map(&inert.as_ref().unwrap());
                create_memo(move || mapped.clone())
            },
            MaybeSignal::Signal(signal) => signal.map(map),
        }
    }
}

impl<T: PartialEq + 'static> SignalMapReactive<T> for MaybeReactive<T> {
    fn map_reactive<U: PartialEq + Clone + 'static>(
        &self,
        map: impl Fn(&T) -> U + 'static,
    ) -> Memo<U> {
        // TODO: Review this
        self.map(map).memo()
    }
}

/// Marker trait for types that statically encode whether a value is reactive
/// or inert. Used as a type-level flag in generic APIs.
///
/// See [`IsReactive`] and [`IsInert`].
pub trait ReactivityMarker {}

/// Marker type indicating that a value is reactive (tracked by the runtime).
/// See [`ReactivityMarker`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct IsReactive;
impl ReactivityMarker for IsReactive {}

/// Marker type indicating that a value is inert (not tracked by the runtime).
/// See [`ReactivityMarker`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct IsInert;
impl ReactivityMarker for IsInert {}

#[cfg(test)]
mod tests {
    use crate::{
        maybe::maybe_reactive::IntoMaybeReactive, prelude::*,
        read::ReadSignal as _, scope::new_deny_new_scope,
    };

    #[test]
    fn static_wrapper() {
        let _deny_new_reactive = new_deny_new_scope();

        let s = 123.inert();
        s.get();
        s.get_cloned();
        s.get_untracked();
        s.map(|s| *s);
    }

    // Type-check tests
    #[test]
    fn conversions() {
        fn accept_maybe_reactive<T: PartialEq + 'static>(
            mr: impl IntoMaybeReactive<T>,
        ) {
            // Assert no reactive value created on conversion
            let _deny_new_reactive = new_deny_new_scope();

            let _ = mr.maybe_reactive();
        }

        // Inert<()>
        // Inert values need explicit conversion into Inert wrapper
        accept_maybe_reactive(().inert());
        // // Derived signal
        // accept_maybe_reactive(|| {});
        // Memo<()>
        accept_maybe_reactive(create_memo(move || {}));
        // Signal<()>
        accept_maybe_reactive(create_signal(()));
        // MemoChain<()>
        accept_maybe_reactive(create_memo_chain(move || {}));
    }

    #[test]
    fn maybe_signal_mapper() {
        let mut maybe = MaybeSignal::new_inert(123);

        // Warning: Non-reactive map
        let map = maybe.map(|value| *value);

        assert_eq!(map.get(), 123);

        maybe.set(234);

        // Map is not reactive
        assert_eq!(map.get(), 123);
        assert!(matches!(map, MaybeReactive::Inert(_)));
    }

    #[test]
    fn maybe_signal_setter_from_reactive() {
        let mut maybe = MaybeSignal::new_inert(123);

        assert_eq!(maybe.get(), 123);

        let reactive = create_signal(69);

        maybe.set_from(reactive.maybe_reactive());

        // Setter turns MaybeSignal into reactive
        assert_eq!(maybe.get(), 69);
        assert!(matches!(maybe, MaybeSignal::Signal(_)));
    }

    // #[test]
    // fn into_maybe_iterator_inert() {
    //     let _deny_new_reactive = new_deny_new_scope();

    //     let items: MaybeReactive<Vec<u32>> =
    //         vec![1, 2, 3].inert().maybe_reactive_iter();
    //     assert_eq!(
    //         items.with(|c| c.iter().cloned().collect::<Vec<_>>()),
    //         vec![1, 2, 3]
    //     );
    // }

    // #[test]
    // fn into_maybe_iterator_inert_generic() {
    //     let _deny_new_reactive = new_deny_new_scope();

    //     fn accept_maybe_iterator(items: impl IntoMaybeReactiveIterator<u32>) {
    //         let items: MaybeReactive<_> = items.maybe_reactive_iter();
    //         assert_eq!(
    //             items.with(|c| c.into_iter().collect::<Vec<_>>()),
    //             vec![1, 2, 3]
    //         );
    //     }
    // }
}

// pub mod inert;
pub mod maybe_reactive;
pub mod maybe_signal;

// Note: The old `SignalMapReactive::map_reactive` trait was removed in WS1.6.
// It was the "make a live Memo from anything, even an inert value" anti-pattern:
// for an inert source it cloned a captured value into a constant memo on every
// read, and for `MaybeReactive` it built two nodes (`.map(map).memo()`). It had
// no call sites. Callers that genuinely need a live memo from a possibly-inert
// source should match on the source and either compute once (inert) or `.map()`
// (reactive) explicitly at the call site.

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

    //     fn accept_maybe_iterator(items: impl IntoMaybeReactiveIterator<u32>)
    // {         let items: MaybeReactive<_> = items.maybe_reactive_iter();
    //         assert_eq!(
    //             items.with(|c| c.into_iter().collect::<Vec<_>>()),
    //             vec![1, 2, 3]
    //         );
    //     }
    // }
}

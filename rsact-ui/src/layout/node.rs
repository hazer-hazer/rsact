use crate::layout::LayoutData;
use core::panic::Location;
use rsact_reactive::{prelude::*, storage::ValueId};

/**
 * Layout is a custom Signal type that is Reactive-on-Write, it means that it is inert until it is set by a reactive source. This is kinda unsafe to use, at least inaccurate in its behavior because in some cases it won't work as expected, because of this, we don't have RoW primitive inside rsact-reactive and declare it here for a specific case of layouts. It MUST not be used by a user and should be used carefully in rsact-ui.
 * Here are the main restrictions of Layout:
 * - Layout does not implement WriteSignal, because `create_effect(|| layout.set(...))` won't work as expected, it will set the layout but won't make it reactive, as we don't know if value in `.set` comes from reactive or inert source.
 */
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Layout {
    Static(ValueId),
    Reactive(Signal<LayoutData>),
}

impl Layout {
    #[track_caller]
    pub(super) fn inert(layout: LayoutData) -> Self {
        let caller = Location::caller();
        // Note: We use Inert value, but it is fake Inert as we write to it, reactive runtime allows this.

        Self::Static(with_current_runtime(|rt| rt.create_inert(layout, caller)))
    }

    #[track_caller]
    fn now_reactive(&mut self) -> Signal<LayoutData> {
        let caller = Location::caller();

        match self {
            Self::Static(inert) => {
                // TODO: rsact-reactive unsafe-denoted method to convert between ValueId reactive types, for Inert -> Signal.
                let signal = with_current_runtime(|rt| -> LayoutData {
                    inert.with_untracked(rt, Clone::clone, caller)
                })
                .signal();

                unsafe { with_current_runtime(|rt| rt.dispose(*inert)) };

                *self = Self::Reactive(signal);

                signal
            },
            Self::Reactive(signal) => *signal,
        }
    }

    // Warn: Untracked! Don't expect to be tracked inside an observer
    #[track_caller]
    pub fn update_untracked(&mut self, f: impl FnOnce(&mut LayoutData)) {
        let caller = Location::caller();

        match self {
            Self::Static(inert) => {
                with_current_runtime(|rt| inert.update_untracked(rt, f, caller))
            },
            Self::Reactive(signal) => {
                signal.update_untracked(|layout| f(layout))
            },
        }
    }
}

impl ReadSignal<LayoutData> for Layout {
    fn track(&self) {
        match self {
            Layout::Static(_) => {},
            Layout::Reactive(signal) => signal.track(),
        }
    }

    #[track_caller]
    fn with_untracked<U>(&self, f: impl FnOnce(&LayoutData) -> U) -> U {
        let caller = Location::caller();

        match self {
            Layout::Static(inert) => {
                with_current_runtime(|rt| inert.with_untracked(rt, f, caller))
            },
            Layout::Reactive(signal) => signal.with_untracked(f),
        }
    }
}

impl<U: PartialEq + 'static> SignalSetter<LayoutData, MaybeReactive<U>>
    for Layout
{
    #[track_caller]
    fn setter(
        &mut self,
        source: MaybeReactive<U>,
        mut set_map: impl FnMut(
            &mut LayoutData,
            &<MaybeReactive<U> as ReactiveValue>::Value,
        ) + 'static,
    ) {
        match source {
            MaybeReactive::Inert(inert) => inert.with(|inert| {
                self.update_untracked(|layout| set_map(layout, inert))
            }),
            MaybeReactive::Memo(memo) => {
                self.now_reactive().setter(memo, set_map)
            },
            MaybeReactive::MemoChain(memo_chain) => {
                self.now_reactive().setter(memo_chain, set_map)
            },
        }
    }
}

impl ReactiveValue for Layout {
    type Value = LayoutData;

    fn id(&self) -> Option<rsact_reactive::storage::ValueId> {
        match self {
            Layout::Static(_) => None,
            Layout::Reactive(signal) => signal.id(),
        }
    }

    fn is_alive(&self) -> bool {
        match self {
            Layout::Static(_) => true,
            Layout::Reactive(signal) => signal.is_alive(),
        }
    }

    #[track_caller]
    unsafe fn dispose(self) {
        match self {
            Layout::Static(_) => {},
            Layout::Reactive(signal) => unsafe { signal.dispose() },
        }
    }
}

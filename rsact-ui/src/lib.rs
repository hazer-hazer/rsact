#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unused_must_use)]

#[macro_use]
extern crate alloc;

// Lets the `#[derive(View)]` macro from `rsact-macros` refer to this crate by
// its real name (`rsact_ui::...`) even from within the crate that defines the
// widgets — the derive emits absolute paths.
extern crate self as rsact_ui;

pub mod anim;
pub mod el;
pub mod event;
pub mod font;
pub mod layout;
pub mod page;
pub mod style;
// Shared headless test/bench scaffolding (WS0.7j); doc-hidden, not public API.
#[doc(hidden)]
pub mod test_support;
pub mod ui;
pub mod utils;
pub mod value;
pub mod widget;

// #[macro_use]
extern crate log;

use rsact_render as render;

pub mod prelude {
    #[cfg(feature = "tiny-icons")]
    pub use crate::widget::icon::*;
    pub use crate::{
        el::*,
        font::FontImport,
        page::id::{PageId, SinglePage},
        style::{
            Style, StylePseudoClass, StyleSelector,
            stylist::{InheritedStylist, InternalStylist, Stylist},
        },
        style::{declare_widget_style, theme::Theme},
        ui::UI,
        widget::{
            button::*, checkbox::*, container::*, dynamic::*, edge::*, flex::*,
            label::*, prelude::*, scrollable::*, select::*, slider::*,
            space::*,
        },
    };

    pub use rsact_render::prelude::*;
    #[cfg(feature = "tiny-icons")]
    pub use rsact_tiny_icons::{IconRaw, IconSet};
}

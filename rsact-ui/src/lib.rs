#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unused_must_use)]

#[macro_use]
extern crate alloc;

pub mod anim;
pub mod el;
pub mod event;
pub mod font;
pub mod layout;
pub mod page;
pub mod style;
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
    pub use rsact_reactive::prelude::*;
    pub use rsact_render::prelude::*;
    #[cfg(feature = "tiny-icons")]
    pub use rsact_tiny_icons::{IconRaw, IconSet};
}

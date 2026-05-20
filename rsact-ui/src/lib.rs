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

#[cfg(feature = "embedded-graphics")]
pub mod eg;

// #[macro_use]
extern crate log;

use rsact_render as render;

pub mod prelude {
    pub use crate::font::FontImport;
    pub use crate::{
        page::id::{PageId, SinglePage},
        style::{declare_widget_style, theme::Theme},
        ui::UI,
        widget::{
            button::*, container::*, edge::*, flex::*, prelude::*,
            scrollable::*, select::*, slider::*, space::*, text::*,
        },
    };
    #[cfg(feature = "embedded-graphics")]
    pub use crate::{
        widget::checkbox::*,
        widget::icon::*,
        // widget::image::*
    };
    #[cfg(feature = "embedded-graphics")]
    pub use rsact_icons::{IconRaw, IconSet};
    pub use rsact_render::prelude::*;
}

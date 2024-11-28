#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;

pub mod anim;
pub mod el;
pub mod event;
pub mod font;
pub mod layout;
pub mod page;
pub mod render;
pub mod style;
pub mod ui;
pub mod utils;
pub mod value;
pub mod widget;

// #[macro_use]
extern crate log;
pub use derivative;
pub use embedded_graphics;
pub use embedded_graphics_core;
pub use embedded_text;

pub mod prelude {
    pub use crate::{
        style::{declare_widget_style, WidgetStylist},
        ui::UI,
        widget::{
            button::*, checkbox::*, container::*, edge::*, flex::*, icon::*,
            image::*, mono_text::*, prelude::*, scrollable::*, select::*,
            slider::*, space::*,
        },
    };
}

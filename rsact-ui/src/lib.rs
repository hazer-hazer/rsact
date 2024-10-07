#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;

pub mod el;
pub mod event;
pub mod font;
pub mod layout;
pub mod page;
pub mod render;
pub mod style;
pub mod ui;
pub mod widget;

// #[macro_use]
extern crate log;

pub mod prelude {
    pub use crate::{
        ui::UI,
        widget::{
            button::*, checkbox::*, container::*, edge::*, flex::*, icon::*,
            image::*, mono_text::*, prelude::*, scrollable::*, select::*,
            slider::*, space::*,
        },
    };
}

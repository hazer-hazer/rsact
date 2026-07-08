#![deny(unused_must_use)]
#![cfg_attr(not(feature = "std"), no_std)]

pub mod prelude {
    // pub use rsact_reactive::prelude::*;
    pub use rsact_ui::prelude::*;
}

// pub use rsact_encoder as encoder;
// pub use rsact_reactive as reactive;
pub use rsact_ui as ui;

/// One-shot render for static / e-paper-class displays (WS3.4). See
/// [`rsact_ui::ui::render_once`].
pub use rsact_ui::ui::render_once;

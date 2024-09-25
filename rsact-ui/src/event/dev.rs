use embedded_graphics::prelude::Point;

pub trait DevElHover {
    fn as_dev_el_hover(&self) -> Option<Point>;
}

pub trait DevToolsToggle {
    fn as_dev_tools_toggle(&self) -> bool;
}

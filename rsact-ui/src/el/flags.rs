#[derive(Debug)]
pub struct WidgetFlags {
    /// Widgets with transparent layout do not have their own layout, so they don't nest layout tree. This is useful for utility widgets like Dynamic. But it does not turn off rendering for the widget, you can still use transparent_layout to avoid creating nested layouts if you definitely need the same layout as a child. Imagine, for example widget that only adds box shadow to a widget, you don't need a separate layout because it would always be equal to the child layout.
    pub transparent_layout: bool,

    // Behavior //
    // TODO: Bitflags?
    pub hoverable: bool,
    pub hoverable_from_children: bool,
    pub bubble_hover: bool,

    pub clickable: bool,
    pub focusable: bool,
    // /// Edge widgets are widgets without children. This flag is generally needed for debugging purposes in cases when something went wrong and layout or other tree mismatches with widget tree.
    // pub is_edge: bool,
}

impl WidgetFlags {
    pub fn transparent_layout(mut self) -> Self {
        self.transparent_layout = true;
        self
    }

    // pub fn is_edge(mut self) -> Self {
    //     self.is_edge = true;
    //     self
    // }
}

impl Default for WidgetFlags {
    fn default() -> Self {
        Self {
            transparent_layout: false,

            hoverable: false,
            // Non-hoverable widget won't receive child hover events, but it is a common default to have.
            hoverable_from_children: true,
            bubble_hover: true,

            clickable: false,
            focusable: false,
        }
    }
}

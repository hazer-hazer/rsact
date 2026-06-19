#[derive(Debug, Clone, Copy)]
pub struct WidgetFlags {
    /// Widgets with transparent layout do not have their own layout, so they don't nest layout tree. This is useful for utility widgets like Dynamic. But it does not turn off rendering for the widget, you can still use transparent_layout to avoid creating nested layouts if you definitely need the same layout as a child. Imagine, for example widget that only adds box shadow to a widget, you don't need a separate layout because it would always be equal to the child layout.
    // TODO: Such transparent flags lead to problems with double-borrow in passes because we first need to check that widget is transparent, then mutate the children and then mutate the widget borrowing it again. Maybe it is better to have real distinct layout of type Transparent that will have the same logic, but it seem to be a larger overhead than double-borrow from arena because we plan to implement composable widget with such flags as [`transparent_layout`] leading us to cases with a lot of nested layouts.
    // [ ] Or maybe better we make a new variant directly in Layout type, allowing us to avoid adding it to the tree at all. No, it's not possible now because it will break child layout dependency. This requires layouts to have separate children storage instead of current reactive tree structure with Container with a single child and Flex with multiple.
    pub transparent_layout: bool,

    // Behavior //
    // TODO: Bitflags?
    pub hoverable: bool,
    pub hoverable_from_children: bool,

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

    pub fn hoverable(mut self) -> Self {
        self.hoverable = true;
        self
    }

    pub fn hoverable_from_children(mut self) -> Self {
        self.hoverable_from_children = true;
        self
    }

    pub fn clickable(mut self) -> Self {
        self.clickable = true;
        self
    }

    pub fn focusable(mut self) -> Self {
        self.focusable = true;
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

            clickable: false,
            focusable: false,
        }
    }
}

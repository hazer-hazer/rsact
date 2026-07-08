use bitflags::bitflags;

bitflags! {
    /// Per-widget behavior flags (WS9a.4: packed into a `u8` via `bitflags`,
    /// was five `bool` fields). Producers build with the chainable setters
    /// (`WidgetFlags::default().hoverable().clickable()`); consumers read with
    /// the `is_*()` accessors.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct WidgetFlags: u8 {
        /// Widgets with transparent layout do not have their own layout, so they
        /// don't nest layout tree. This is useful for utility widgets like
        /// Dynamic. But it does not turn off rendering for the widget, you can
        /// still use transparent_layout to avoid creating nested layouts if you
        /// definitely need the same layout as a child. Imagine, for example widget
        /// that only adds box shadow to a widget, you don't need a separate layout
        /// because it would always be equal to the child layout.
        // TODO: Such transparent flags lead to problems with double-borrow in
        // passes because we first need to check that widget is transparent, then
        // mutate the children and then mutate the widget borrowing it again. Maybe
        // it is better to have real distinct layout of type Transparent that will
        // have the same logic, but it seem to be a larger overhead than
        // double-borrow from arena because we plan to implement composable widget
        // with such flags as [`transparent_layout`] leading us to cases with a lot
        // of nested layouts. [ ] Or maybe better we make a new variant
        // directly in Layout type, allowing us to avoid adding it to the tree at
        // all. No, it's not possible now because it will break child layout
        // dependency. This requires layouts to have separate children storage
        // instead of current reactive tree structure with Container with a single
        // child and Flex with multiple.
        const TRANSPARENT_LAYOUT = 1 << 0;

        // Behavior //
        const HOVERABLE = 1 << 1;
        const HOVERABLE_FROM_CHILDREN = 1 << 2;
        const CLICKABLE = 1 << 3;
        const FOCUSABLE = 1 << 4;
        // /// Edge widgets are widgets without children. This flag is generally
        // needed for debugging purposes in cases when something went wrong and
        // layout or other tree mismatches with widget tree. const IS_EDGE = 1 << 5;
    }
}

impl WidgetFlags {
    // Chainable setters (producer side) — return `Self` so widget `flags()`
    // impls read as `WidgetFlags::default().hoverable().clickable()`.
    pub fn transparent_layout(self) -> Self {
        self | Self::TRANSPARENT_LAYOUT
    }

    pub fn hoverable(self) -> Self {
        self | Self::HOVERABLE
    }

    pub fn hoverable_from_children(self) -> Self {
        self | Self::HOVERABLE_FROM_CHILDREN
    }

    pub fn clickable(self) -> Self {
        self | Self::CLICKABLE
    }

    pub fn focusable(self) -> Self {
        self | Self::FOCUSABLE
    }

    // pub fn is_edge(self) -> Self {
    //     self | Self::IS_EDGE
    // }

    // Accessors (consumer side).
    pub fn is_transparent_layout(self) -> bool {
        self.contains(Self::TRANSPARENT_LAYOUT)
    }

    pub fn is_hoverable(self) -> bool {
        self.contains(Self::HOVERABLE)
    }

    pub fn is_hoverable_from_children(self) -> bool {
        self.contains(Self::HOVERABLE_FROM_CHILDREN)
    }

    pub fn is_clickable(self) -> bool {
        self.contains(Self::CLICKABLE)
    }

    pub fn is_focusable(self) -> bool {
        self.contains(Self::FOCUSABLE)
    }
}

impl Default for WidgetFlags {
    fn default() -> Self {
        // Non-hoverable widget won't receive child hover events, but it is
        // a common default to have.
        Self::HOVERABLE_FROM_CHILDREN
    }
}

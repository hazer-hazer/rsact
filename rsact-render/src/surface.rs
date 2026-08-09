use crate::{geometry::Size, renderer::ViewportKind};
use alloc::vec::Vec;

pub trait Surface {
    fn new(size: Size) -> Self;
}

/// A single drawing [`Framebuffer`] plus a stack of nested clip/crop viewports.
///
/// Layer compositing was removed — it was a badly-fitting model (a `Vec` of
/// full-screen surfaces blended per frame). Nested composition, when needed,
/// will come from tree-depth composition, not a surface stack. What remains is
/// the viewport (clip/crop) stack that `clipped()`/`cropped()` push and pop
/// while a subtree renders.
pub struct Canvas<T: Surface> {
    surface: T,
    viewport_stack: Vec<ViewportKind>,
}

impl<T: Surface> Canvas<T> {
    pub fn new(size: Size) -> Self {
        Self {
            surface: T::new(size),
            viewport_stack: vec![ViewportKind::root()],
        }
    }

    pub fn current_viewport(&self) -> ViewportKind {
        *self.viewport_stack.last().unwrap()
    }

    pub fn surface(&self) -> &T {
        &self.surface
    }

    pub fn surface_mut(&mut self) -> &mut T {
        &mut self.surface
    }

    /// Push a nested viewport, **narrowed by the one already active**
    /// (`ViewportKind::nested_in`) so the top of the stack is always the
    /// EFFECTIVE clip. Before WS6.4b this stored `kind` raw, and since the write
    /// filters consult only the top, a clip wider than its parent widened the
    /// effective clip.
    pub fn enter_viewport(&mut self, kind: ViewportKind) {
        let nested = kind.nested_in(self.current_viewport());
        self.viewport_stack.push(nested);
    }

    pub fn exit_viewport(&mut self) {
        self.viewport_stack.pop();
    }
}

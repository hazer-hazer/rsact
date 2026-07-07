use crate::{
    geometry::Size,
    renderer::{Viewport, ViewportKind},
};
use alloc::vec::Vec;

pub trait Surface {
    fn new(size: Size) -> Self;
}

pub struct Layer<T: Surface> {
    surface: T,
}

pub struct Layering<T: Surface> {
    viewport_stack: Vec<Viewport>,
    // 9a.2: sorted `Vec` keyed by layer index instead of a `BTreeMap`. Dynamic
    // insertion is currently disabled (see the commented `on_layer` path) so
    // N == 1, but the vec stays sorted by index so compositing order (ascending)
    // is preserved if layering is reinstated.
    layers: Vec<(usize, Layer<T>)>,
}

impl<T: Surface> Layering<T> {
    pub fn new(size: Size) -> Self {
        Self {
            viewport_stack: vec![Viewport::root()],
            layers: vec![(0, Layer { surface: T::new(size) })],
        }
    }

    pub fn current_viewport(&self) -> &Viewport {
        self.viewport_stack.last().unwrap()
    }

    pub fn layer_index(&self) -> usize {
        self.current_viewport().layer
    }

    pub fn surface_mut(&mut self) -> &mut T {
        let idx = self.layer_index();
        let pos =
            self.layers.binary_search_by_key(&idx, |(k, _)| *k).unwrap();
        &mut self.layers[pos].1.surface
    }

    pub fn layers_mut(&mut self) -> impl Iterator<Item = &mut T> {
        // Sorted by index, so iteration is already in compositing order.
        self.layers.iter_mut().map(|(_, layer)| &mut layer.surface)
    }

    fn sub_viewport(&self, kind: ViewportKind) -> Viewport {
        let parent = self.current_viewport();
        Viewport { layer: parent.layer, kind }
    }

    pub fn enter_viewport(&mut self, kind: ViewportKind) {
        self.viewport_stack.push(self.sub_viewport(kind));
    }

    pub fn exit_viewport(&mut self) {
        self.viewport_stack.pop();
    }

    // fn with_viewport(
    //     &mut self,
    //     kind: ViewportKind,
    //     f: impl FnOnce(&mut Self) -> RenderResult,
    // ) -> RenderResult {
    //     self.viewport_stack.push(self.sub_viewport(kind));
    //     let result = f(self);
    //     self.viewport_stack.pop();
    //     result
    // }

    // pub fn clipped(
    //     &mut self,
    //     area: &Rect,
    //     f: impl FnOnce(&mut Self) -> RenderResult,
    // ) -> RenderResult {
    //     self.with_viewport(ViewportKind::Clipped(area.clone()), f)
    // }

    // pub fn cropped(
    //     &mut self,
    //     area: &Rect,
    //     f: impl FnOnce(&mut Self) -> RenderResult,
    // ) -> RenderResult {
    //     self.with_viewport(ViewportKind::Cropped(area.clone()), f)
    // }
}

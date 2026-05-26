use crate::{
    geometry::Size,
    renderer::{Viewport, ViewportKind},
};
use alloc::{collections::btree_map::BTreeMap, vec::Vec};

pub trait Surface {
    fn new(size: Size) -> Self;
}

pub struct Layer<T: Surface> {
    surface: T,
}

pub struct Layering<T: Surface> {
    viewport_stack: Vec<Viewport>,
    layers: BTreeMap<usize, Layer<T>>,
}

impl<T: Surface> Layering<T> {
    pub fn new(size: Size) -> Self {
        Self {
            viewport_stack: vec![Viewport::root()],
            layers: BTreeMap::from([(0, Layer { surface: T::new(size) })]),
        }
    }

    pub fn current_viewport(&self) -> &Viewport {
        self.viewport_stack.last().unwrap()
    }

    pub fn layer_index(&self) -> usize {
        self.current_viewport().layer
    }

    pub fn surface_mut(&mut self) -> &mut T {
        &mut self.layers.get_mut(&self.layer_index()).unwrap().surface
    }

    pub fn layers_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.layers.values_mut().map(|layer| &mut layer.surface)
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

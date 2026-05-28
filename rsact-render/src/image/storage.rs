use crate::{
    color::Color,
    image::{ImageOwned, ImageRef},
};
use slotmap::{Key, SlotMap};

// TODO: Custom IDs can be a problem when we add a resource loading functionality, as this would require doubling this, what I don't want to.
/// Simple generic storage for images that can be used by renderers to store and reference images by ID. This allows to use images in reactive context where direct usage of image reference would require 'static lifetime.
/// Custom ID is required for safety. Because there can appear multiple ImageStorage's and we want to prevent ID mixing between different image sources. The downside of this approach is that images from main UI ImageStorage cannot be used in Canvas and vice versa.
pub struct ImageStorage<K: Key, C: Color> {
    images: SlotMap<K, ImageOwned<C>>,
}

impl<K: Key, C: Color> ImageStorage<K, C> {
    pub fn new() -> Self {
        Self { images: SlotMap::default() }
    }

    pub fn add(&mut self, image: ImageOwned<C>) -> K {
        self.images.insert(image)
    }

    pub fn get(&self, id: K) -> Option<ImageRef<'_, C>> {
        self.images.get(id).map(|image| image.as_ref())
    }
}

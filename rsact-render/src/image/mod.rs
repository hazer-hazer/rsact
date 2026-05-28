use crate::{
    color::Color,
    geometry::{Point, Rect, Size},
};
use alloc::boxed::Box;
use core::marker::PhantomData;

pub mod storage;

#[derive(Debug, Clone)]
pub enum Image<'a, K: slotmap::Key, C: Color> {
    Owned(ImageOwned<C>),
    Ref(ImageRef<'a, C>),
    Id(K),
}

impl<'a, K: slotmap::Key, C: Color> From<ImageOwned<C>> for Image<'a, K, C> {
    fn from(image: ImageOwned<C>) -> Self {
        Self::Owned(image)
    }
}

impl<'a, K: slotmap::Key, C: Color> From<ImageRef<'a, C>> for Image<'a, K, C> {
    fn from(image: ImageRef<'a, C>) -> Self {
        Self::Ref(image)
    }
}

impl<'a, K: slotmap::Key, C: Color> From<K> for Image<'a, K, C> {
    fn from(id: K) -> Self {
        Self::Id(id)
    }
}

impl<'a, K: slotmap::Key, C: Color> PartialEq for Image<'a, K, C> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            // We assume that two different owned images are always different, even if they have the same content.
            (Self::Owned(_), Self::Owned(_)) => false,
            (Self::Ref(l0), Self::Ref(r0)) => l0 == r0,
            (Self::Id(l0), Self::Id(r0)) => l0 == r0,
            _ => false,
        }
    }
}

pub struct DrawImage<'a, C: Color> {
    image: ImageRef<'a, C>,
    position: Point,
}

impl<'a, C: Color> DrawImage<'a, C> {
    pub fn new(image: ImageRef<'a, C>, position: Point) -> Self {
        Self { image, position }
    }

    pub const fn image(&self) -> &ImageRef<'a, C> {
        &self.image
    }

    pub const fn size(&self) -> Size {
        self.image.size()
    }

    pub const fn bounding_box(&self) -> Rect {
        Rect { top_left: self.position, size: self.size() }
    }

    pub const fn data(&self) -> &'a [u8] {
        self.image.data()
    }

    pub const fn position(&self) -> Point {
        self.position
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ImageRef<'a, C: Color> {
    data: &'a [u8],
    size: Size,
    _color: PhantomData<C>,
}

impl<'a, C: Color> PartialEq for ImageRef<'a, C> {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data && self.size == other.size
    }
}

impl<'a, C: Color> ImageRef<'a, C> {
    pub const fn size(&self) -> Size {
        self.size
    }

    pub const fn data(&self) -> &'a [u8] {
        self.data
    }
}

#[derive(Debug, Clone)]
pub struct ImageOwned<C: Color> {
    data: Box<[u8]>,
    size: Size,
    _color: PhantomData<C>,
}

impl<C: Color> ImageOwned<C> {
    pub const fn new(data: Box<[u8]>, size: Size) -> Self {
        Self { data, size, _color: PhantomData }
    }

    pub const fn size(&self) -> Size {
        self.size
    }

    pub const fn data(&self) -> &[u8] {
        &self.data
    }

    pub const fn as_ref(&self) -> ImageRef<'_, C> {
        ImageRef { data: &self.data, size: self.size, _color: PhantomData }
    }
}

#[cfg(feature = "tiny-skia")]
impl<C: Color> From<tiny_skia::Pixmap> for ImageOwned<C> {
    fn from(pixmap: tiny_skia::Pixmap) -> Self {
        let size = Size::new(pixmap.width(), pixmap.height());
        let data = pixmap.take().into_boxed_slice();
        Self::new(data, size)
    }
}

use core::marker::PhantomData;

pub struct ReadSignal<T: 'static> {
    id: Value,
    ty: PhantomData<T>,
}

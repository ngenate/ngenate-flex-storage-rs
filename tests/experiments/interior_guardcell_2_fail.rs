// This experiment demonstrates that parking lots Mutex type does not implement Coerce Unsized
// which means that we can't create references to trait objects that are implemented for guards
// such as MutexGuard or RwLockReadGuard. In summary this means that we can't get references or
// Boxed unsized types to the trait objects that we are interested in. ie. InnerIterStorage
//
// This experiment fails because the only way to achieve this with safe code is to use parking lots
// MappedGuard types which can be seen working in the other experiment. 

#[cfg(test)]
mod tests
{
    use parking_lot::{Mutex, MutexGuard};
    use std::{marker::PhantomData, ops::Deref};

    struct VecStorage<Item>
    {
        data: Mutex<Box<Vec<Item>>>,
        _phantom: PhantomData<Item>,
    }

    trait OuterIterStorage
    {
        type Item;

        fn borrow_inner_storage(
            &self,
        ) -> Box<dyn Deref<Target = dyn InnerIterStorage<Item = Self::Item>> + '_>;
    }

    trait InnerIterStorage
    {
        type Item;

        fn as_iter(&self) -> Box<dyn Iterator<Item = &Self::Item> + '_>;
    }

    // Note we implement this on Box<Vec<T>> instead of plain Vec<T> to play
    // better with MappedMutexGuard as it doesn't seem to be possible otherwise.
    impl<T> InnerIterStorage for Box<Vec<T>>
    {
        type Item = T;

        fn as_iter(&self) -> Box<dyn Iterator<Item = &Self::Item> + '_>
        {
            Box::new(self.iter())
        }
    }

    impl<Item> OuterIterStorage for VecStorage<Item>
    where
        Item: 'static,
    {
        type Item = Item;

        fn borrow_inner_storage(
            &self,
        ) -> Box<dyn Deref<Target = dyn InnerIterStorage<Item = Item>> + '_>
        {
            let guard: MutexGuard<Box<Vec<Item>>> = self.data.lock();

            // Both of these coercion attempts to trait objects dont work because coerce unsized is
            // not implemented for parking lots MutexGuard Its also not currently
            // implemented for std::sync::Mutex or RwLock either so it will be the same issue there

            // let guard: &dyn Deref<Target = dyn InnerIterStorage<Item = Self::Item>> = &guard;
            // let guard: Box<dyn Deref<Target = dyn InnerIterStorage<Item = Self::Item>> + '_> =
            // Box::new(guard);

            // But it does work for Deref into the concrete inner types using either of:
            // let guard: &dyn Deref<Target = Box<Vec<Item>>> = &guard;
            let guard: Box<dyn Deref<Target = Box<Vec<Item>>>> = Box::new(guard);

            // But we want to return the trait object not the concrete type
            // so we can't return the above either.

            guard
        }
    }

    #[test]
    fn test()
    {
        let storage: VecStorage<i32> = VecStorage {
            data: Mutex::new(Box::new(vec![1, 2, 3])),
            _phantom: <_>::default(),
        };

        let outer_storage: &dyn OuterIterStorage<Item = i32> = &storage;

        let inner_storage_borrow: Box<dyn Deref<Target = dyn InnerIterStorage<Item = i32>> + '_> =
            outer_storage.borrow_inner_storage();

        let iter = inner_storage_borrow.as_iter();

        for i in iter
        {
            dbg!(i);
        }
    }
}

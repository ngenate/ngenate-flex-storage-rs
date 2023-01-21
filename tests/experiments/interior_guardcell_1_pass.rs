// This experiment demonstrates that it is possible to perform dynamic dispatch on a storage type that has a RwLock on its interior.
// The motivation to have a RwLock on the inside of a storage is that it improves compatibility with being able to be downcast 
// via Any or crates such as downncast_rs since these cannot downcast with types such as Arc<RwLock<dyn Storage>>.
// It is not straight forward to return a dynamic dispatched trait object from within the RwLock because RwLock / Mutex guards 
// from either the std::sync or parking lot do not currently implement Coerce unsized. 
// So instead we need to use parking lots MappedMutexGuard which lets us return a boxed guard with a mapping function 
// that lets us Deref to the desired trait object target. 
// Another requirement to get this all working is that two families of trait objects are needed. An outer set to perform 
// requests to acquire locks from the interior cell (Mutex / RwLock) and an inner trait object to allow for dynamic dispatch 
// based operations to be carried out on the interior data once we have acquired the lock.
// Having two sets of trait objects for a simple storage crate is probably acceptable but for our library there is quite a lot 
// of traits which would need to be doubled again if going down this path.
// The main alternative is to have the RwLock / Mutex outside your storage type and use some unsafe code which this crate has 
// currently chosen to use to reduce the above complexity.
// note: The parking lot crate is needed because it has MappedGuards which std::sync types lack.

#[cfg(test)]
mod tests
{
    use parking_lot::{MappedMutexGuard, Mutex, MutexGuard};
    use std::{marker::PhantomData, ops::Deref};

    struct VecStorage<Item> {
        data: Mutex<Box<Vec<Item>>>,
        _phantom: PhantomData<Item>,
    }

    trait OuterIterStorage {
        type Item;

        fn borrow_inner_storage(
            &self,
        ) -> Box<dyn Deref<Target = dyn InnerIterStorage<Item = Self::Item>> + '_>;
    }

    trait InnerIterStorage {
        type Item;

        fn as_iter(&self) -> Box<dyn Iterator<Item = &Self::Item> + '_>;
    }

    // Note we implement this on Box<Vec<T>> instead of plain Vec<T> to play
    // better with MappedMutexGuard as it doesn't seem to be possible otherwise.
    impl<T> InnerIterStorage for Box<Vec<T>> {
        type Item = T;

        fn as_iter(&self) -> Box<dyn Iterator<Item = &Self::Item> + '_> {
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
        ) -> Box<dyn Deref<Target = dyn InnerIterStorage<Item = Item>> + '_> {
            let guard = self.data.lock();

            let mapped: MappedMutexGuard<dyn InnerIterStorage<Item = Item>> =
                MutexGuard::map(guard, |v| v as &mut dyn InnerIterStorage<Item = Item>);

            Box::new(mapped)
        }
    }

    #[test]
    fn test() {
        let storage: VecStorage<i32> = VecStorage {
            data: Mutex::new(Box::new(vec![1, 2, 3])),
            _phantom: <_>::default(),
        };

        let outer_storage: &dyn OuterIterStorage<Item = i32> = &storage;

        let inner_storage_borrow: Box<dyn Deref<Target = dyn InnerIterStorage<Item = i32>> + '_> =
            outer_storage.borrow_inner_storage();

        let iter = inner_storage_borrow.as_iter();

        for i in iter {
            dbg!(i);
        }
    }
}

// The following is a more complete version of essentially what was demonstrated in the interior
// guard cell parking lot mapped guard experiment. However this is fleshed out into a mini POC
// and interestingly in this case RefCell is used which unlike all RwLock / Mutex types that I've 
// seen, actually does implement CoerceUnsized which is why we don't need to use any special kind 
// of mapping function here to get at the internal trait object.
// Ultimately though this experiment still suffers from some issues as it only works in single threaded 
// due to RefCell and it requires two related sets of traits which I called Exterior and Interior traits 
// in another experiment. In this example IterStorage and SliceStorage are the exterior traits that 
// facilitate acquiring the borrow / lock on the guard. And IterAccess and SliceAccess are the interior 
// traits that once we have the borrow let us actually do stuff using dynamic dispatch without needing 
// to re borrow each time. 
// So in conclusion, if this approach was converted to use parking lots mapped guard technique AND 
// you are happy with the overhead of maintaining TWO related sets of traits then this is a safe way 
// to achieve a storage framework with lots of shared traits and that allows inter trait casting 
// with safe code due to the guard type being on the inside.
// But again, in my mind having a small amount of unsafe code and RwLock on the outside to do away
// with the two trait hierarchies seems to win.

use std::{
    any::TypeId,
    cell::{Ref, RefCell},
    rc::Rc,
};

use downcast_rs::{impl_downcast, Downcast};

impl_downcast!(Storage);

pub trait IterAccess
{
    type Item;

    fn as_iter(&self) -> Box<dyn Iterator<Item = &Self::Item> + '_>;
}

pub trait SliceAccess
{
    type Item;

    fn as_slice(&self) -> &[Self::Item];
}

pub trait Storage: Downcast
{
    fn id(&self) -> u64;

    fn item_type_id(&self) -> TypeId;
}

pub trait IterStorage: Storage
{
    type Item;

    fn iter_access(&self) -> Ref<dyn IterAccess<Item = Self::Item>>;
}

pub trait SliceStorage: Storage
{
    type Item;

    fn slice_access(&self) -> Ref<dyn SliceAccess<Item = Self::Item>>;
}

pub struct VecStorage<T>
{
    id: u64,
    data: RefCell<Vec<T>>,
}

impl<T> VecStorage<T>
{
    pub fn new(id: u64) -> Self
    where
        T: 'static,
    {
        let data: RefCell<Vec<T>> = RefCell::new(Vec::new());

        Self { id, data }
    }

    pub fn new_from_iter<I: IntoIterator<Item = T>>(id: u64, iter: I) -> Self
    {
        let mut vec: Vec<T> = Vec::new();

        for i in iter
        {
            vec.push(i);
        }

        VecStorage {
            id,
            data: RefCell::new(vec),
        }
    }

    pub fn borrow_data(&self) -> Ref<Vec<T>>
    {
        self.data.borrow()
    }
}

impl<T> Storage for VecStorage<T>
where
    T: 'static,
{
    fn item_type_id(&self) -> TypeId
    {
        TypeId::of::<T>()
    }

    fn id(&self) -> u64
    {
        self.id
    }
}

impl<T> IterStorage for VecStorage<T>
where
    T: 'static,
{
    type Item = T;

    fn iter_access(&self) -> Ref<dyn IterAccess<Item = Self::Item>>
    {
        self.borrow_data()
    }
}

impl<T> SliceStorage for VecStorage<T>
where
    T: 'static,
{
    type Item = T;

    fn slice_access(&self) -> Ref<dyn SliceAccess<Item = Self::Item>>
    {
        self.borrow_data()
    }
}

impl<T> IterAccess for Vec<T>
{
    type Item = T;

    fn as_iter(&self) -> Box<dyn Iterator<Item = &Self::Item> + '_>
    {
        Box::new(self.iter())
    }
}

impl<T> SliceAccess for Vec<T>
{
    type Item = T;

    fn as_slice(&self) -> &[Self::Item]
    {
        &self
    }
}

// Cross cast a storage trait object to an IterStorage trait object.
// Returns None if the cast is not possible.
// This is made possible by first downcasting and then upcasting
// as rust does not allow for direct inter trait casting as a language
// feature.
pub fn downcast_to_iter_storage<Item>(
    storage: &Rc<dyn Storage>,
) -> Option<Rc<dyn IterStorage<Item = Item>>>
where
    Item: 'static,
{
    if TypeId::of::<Item>() != storage.item_type_id()
    {
        return None;
    }

    let storage = storage.clone();

    if let Ok(rc) = storage.downcast_rc::<VecStorage<Item>>()
    {
        let iter_storage: Rc<dyn IterStorage<Item = Item>> = rc;

        return Some(iter_storage);
    }

    None
}

/// Downcast the storage to it's concrete type
pub fn downcast_storage_to_type<StorageType>(storage: &Rc<dyn Storage>) -> Option<Rc<StorageType>>
where
    StorageType: Storage + 'static,
{
    let ds = storage.clone();

    if let Ok(rc) = ds.downcast_rc::<StorageType>()
    {
        return Some(rc);
    }

    None
}

#[cfg(test)]
mod tests
{
    use std::rc::Rc;

    use super::{downcast_storage_to_type, downcast_to_iter_storage, IterStorage, Storage};

    use super::VecStorage;

    #[test]
    fn test_storage_downcast()
    {
        let vec_storage: VecStorage<i32> = VecStorage::new(0);

        let rc: Rc<dyn Storage> = Rc::new(vec_storage);

        if let Some(rc) = downcast_storage_to_type::<VecStorage<i32>>(&rc)
        {
            let iter_storage: &dyn IterStorage<Item = i32> = rc.as_ref();

            for i in iter_storage.iter_access().as_iter()
            {
                dbg!(i);
            }
        }
    }

    #[test]
    fn test_downcast_to_iter_storage()
    {
        let vec = vec![1, 2, 3];
        let vec_storage = VecStorage::new_from_iter(0, vec);

        let rc: Rc<dyn Storage> = Rc::new(vec_storage);

        if let Some(rc) = downcast_to_iter_storage::<i32>(&rc)
        {
            for i in rc.iter_access().as_iter()
            {
                dbg!(i);
            }
        }
    }
}

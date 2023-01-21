
use std::{any::TypeId, marker::PhantomData};

use guardian::{ArcRwLockReadGuardian, ArcRwLockWriteGuardian};
use sendable::SendOption;

use crate::{
    casting::dyn_storage_into_sized,
    storage_traits::{
        ClearableStorage, ItemStorage, ItemTrait, ItemTypeIdNoSelf, KeyItemStorage,
        KeyStorage, KeyTrait, KeyTypeIdNoSelf, MutKeyItemStorage, Storage, ViewStorageSetup,
    },
    Arw, OArw, SimpleResult, storage_types::key_to_index,
};

/// Provides a view into any other storage that implements [KeyItemStorage]
///
/// # When to use
/// Use storage views when you need to select sub sets of items and the memory overhead of copying
/// or cloning those item to a new Storage is too great.
///
/// # Use cases
/// You have a storage containing a large tuple of data representing columns from a database. After
/// running an iterator filter function you might have reduced a dataset of 10k of these items by
/// just 10 items.
///
/// With an iterator you will need to process these items right then and there in order to avoid
/// cloning them / allocating memory to this smaller subset of items to be able to process them
/// later from a heap location.
///
/// This view side steps that limitation by allowing a filtered view on a dataset to continue to
/// live on the heap as long as u need it. This is very useful for dataflow processing through a
/// node graph where data may need to live on the heap and be associated with each node via heap
/// allocated storage types
///
/// # Limitation
/// If you are explicitly after a view that is a SubSlice of another storage then you will need a
/// special Slice only view type that is not implemented yet as this view type is just intended for
/// Key, Item based views into other storages. You could use this as a substitute for that in the
/// meantime or just clone out the sub section that you need into a new VecStorage for example.
//
// -------------------------------------------------------------------------------
//
// # Internal Design
//
// ## Guardian locks instead of RwLockRead/WriteGuard
//
// Guardian is required because it allows us to validate that we have acquired a lock on a dependency
// once before using the rest of the StorageViews API. and then only after that invariant has been
// proved we can access the rest of ViewStorages API with as many successive calls as we like
// without trying to re-acquire the lock for each call. This two main steps are controlled or gated
// via the [ViewStorageGate] which prevents the user from accessing this ViewStorage until a
// guardian lock an input storage has been taken out.
//
// ## Send Option
//
// SendOption from crates.io -> Sendable is required because guardians guards have a !Send
// Constraint which also happens to be on guards from the Mutex and plain RwLock guards too.
// This constraint is important because it is UB to send an lock that was acquired on one thread
// and then have another thread try to unlock it on drop.
//
// There is a tracking (issue)[https://github.com/rust-lang/rust/issues/93740],
// soon to conclude that is improving std:sync types which is taking a lot of inspiration from
// parking lot There is a point listed under "possible later goals" which states:
// "Allow Sending MutexGuards to other threads"
//
// ## Excluded Trait Implementations
//
// [ValSliceAccess] is deliberately not implemented for RefViewStorage.
// Since this view only stores Keys that may not map to contiguous memory for items in the input
// storage, its not possible to return such a mapped slice. The Keys are contiguous and can be
// returned as a slice but instead of doing this via the [ValSliceAccess] trait, this is done via
// impl methods on the type with an appropriate name to avoid the expectation from the user that
// they should be getting a slice of selected items only.
//
// Likewise [AsBytesBorrowed] is not implemented for this due to the non sequential memory
// layout of the pointers stored in data
#[derive(Default)]
pub struct KeyItemViewStorage<InputStorage, Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
    InputStorage: KeyItemStorage<Key = Key, Item = Item>,
{
    view_keys: Vec<Key>,
    input_storage: OArw<InputStorage>,

    read_guard: SendOption<ArcRwLockReadGuardian<InputStorage>>,
    write_guard: SendOption<ArcRwLockWriteGuardian<InputStorage>>,
}

////////////////////////////////////////////////////////////////////////////////
// Inherent methods
////////////////////////////////////////////////////////////////////////////////

impl<InputStorage, Key, Item> KeyItemViewStorage<InputStorage, Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
    InputStorage: KeyItemStorage<Key = Key, Item = Item>,
{
    pub fn new() -> Self
    {
        Self {
            view_keys: <_>::default(),
            input_storage: <_>::default(),
            read_guard: <_>::default(),
            write_guard: <_>::default(),
        }
    }

    fn as_keys_slice(&self) -> &[Key]
    {
        self.view_keys.as_slice()
    }

    /// Create an iterator returns tuples of (Key, &Item).
    fn key_item_iter_static(&self) -> KeysToItemsIter<'_, InputStorage, std::slice::Iter<'_, Key>, Item>
    {
        // Attempt to get an iterator from any guard that is available out of the read and write guards
        // panicking of no guards are available

        if let Some(input_storage) = self.read_guard.as_ref() {

            let iter: KeysToItemsIter<InputStorage, std::slice::Iter<Key>, Item> =
                KeysToItemsIter::new(input_storage, self.view_keys.iter());

            return iter;
        };

        if let Some(input_storage) = self.write_guard.as_ref() {

            let iter: KeysToItemsIter<InputStorage, std::slice::Iter<Key>, Item> =
                KeysToItemsIter::new(input_storage, self.view_keys.iter());

            return iter;
        };

        panic!("Cannot create an iterator without first creating view data");
    }
}

// ---------------------------------------------------------------
// Storage Supertrait implements
// ---------------------------------------------------------------

impl<InputStorage, Key, Item> ItemStorage for KeyItemViewStorage<InputStorage, Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
    InputStorage: KeyItemStorage<Key = Key, Item = Item>,
{
    type Item = Item;
}

impl<InputStorage, Key, Item> KeyStorage for KeyItemViewStorage<InputStorage, Key, Item>
where
    Key: KeyTrait, // + Into<usize>,
    Item: ItemTrait,
    InputStorage: KeyItemStorage<Key = Key, Item = Item>,
{
    type Key = Key;

    fn contains(&self, key: Self::Key) -> bool
    {
        if let Some(input_data_guard) = &*self.read_guard
        {
            let entry: Option<&Key> = self.view_keys.get(key_to_index(key));

            if let Some(index) = entry
            {
                input_data_guard.contains(*index)
            }
            else
            {
                false
            }
        }
        else
        {
            false
        }
    }

    fn keys_iter(&self) -> Box<dyn Iterator<Item = Self::Key> + '_>
    {
        Box::new(self.view_keys.iter().cloned())
    }
}

impl<InputStorage, Key, Item> KeyItemStorage for KeyItemViewStorage<InputStorage, Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
    InputStorage: KeyItemStorage<Key = Key, Item = Item>,
{
    fn get(&self, key: Self::Key) -> Option<&Item>
    {
        // Orig that only worked with a read_guard and not EITHER a read or write guard as the new one does below
        // Leaving for reference for a while but TODO: Delete soon.
        // if let Some(input_data_guard) = &*self.read_guard
        // {
        //     let entry: Option<&Key> = self.view_data.get(key_to_index(key));
        //     if let Some(index) = entry
        //     {
        //         input_data_guard.get(*index)
        //     }
        //     else
        //     {
        //         None
        //     }
        // }
        // else
        // {
        //     None
        // }

        // Get using any guard that is active (either the read or write guard)
        // otherwise return None

        if let Some(input_data_guard) = self.read_guard.as_ref() {

            let entry: Option<&Key> = self.view_keys.get(key_to_index(key));
            if let Some(index) = entry
            {
                return input_data_guard.get(*index);
            }
            else
            {
                return None;
            }
        };

        if let Some(input_data_guard) = self.write_guard.as_ref() {

            let entry: Option<&Key> = self.view_keys.get(key_to_index(key));
            if let Some(index) = entry
            {
                return input_data_guard.get(*index);
            }
            else
            {
                return None;
            }

        };

        None
    }

    fn item_iter(&self) -> Box<dyn Iterator<Item = &Self::Item> + '_> {

        let iter = self.key_item_iter_static()
            .map(|(_, item)| item);

        Box::new(iter)
    }

    fn key_item_iter(&self) -> Box<dyn Iterator<Item = (Self::Key, &Self::Item)> + '_>
    {
        let iter = self.key_item_iter_static();
        Box::new(iter)
    }
}

impl<InputStorage, Key, Item> MutKeyItemStorage for KeyItemViewStorage<InputStorage, Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
    InputStorage: MutKeyItemStorage<Key = Key, Item = Item>,
{
    fn get_mut(&mut self, key: Self::Key) -> Option<&mut Item>
    {
        if let Some(input_data_guard) = &mut *self.write_guard
        {
            let entry: Option<&Key> = self.view_keys.get(key_to_index(key));
            if let Some(index) = entry
            {
                input_data_guard.get_mut(*index)
            }
            else
            {
                None
            }
        }
        else
        {
            None
        }
    }

    /// Insert the item at the key location overwriting any existing item.
    /// # Panics
    /// This will panic if the key is not part of the view already
    /// TODO: #LOW Return an error instead of panicking 
    fn insert(&mut self, key: Self::Key, item: Self::Item)
    {
        let Some(existing_item) = self.get_mut(key) 
        else {panic!("Could not insert item at key location as the view does not already contain this key")};

        *existing_item = item;
    }
}

// ---------------------------------------------------------------
// Storage trait family impl
// ---------------------------------------------------------------

impl<InputStorage, Key, Item> ViewStorageSetup for KeyItemViewStorage<InputStorage, Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
    InputStorage: KeyItemStorage<Key = Key, Item = Item>,
{
    fn clear_view(&mut self) {

        // Clear the view keys
        self.view_keys.clear();

        // Drop the guards
        self.read_guard = <_>::default();
        self.write_guard = <_>::default();
    }

    fn set_input_storage(&mut self, input: Arw<dyn Storage>)
    {
        // input storage is potentially changed so we need to clear
        // this to be safe
        self.clear_view();

        // Cast to the concrete version
        // -----------------------------------------------------------------
        // This makes all the interior content of the view concrete which is
        // important for us later being able to get a concrete version of the
        // view and know that its interior is concrete and thus good to
        // go for static dispatch
        let storage: Arw<InputStorage> =
            dyn_storage_into_sized::<dyn Storage, InputStorage>(input).unwrap();

        self.input_storage = Some(storage);
    }

    fn get_input_storage(&self) -> Option<Arw<dyn Storage>>
    {
        let Some(input) = self.input_storage.clone() else {
            return None
        };

        let storage: Arw<dyn Storage> = input;

        Some(storage)
    }

    fn create_read_view(&mut self, keys: Box<dyn Iterator<Item = Key>>) -> SimpleResult<()>
    {
        let Some(input) = &self.input_storage else {
            return Err("Input storage not set".into());
        };

        let Ok(guard) = ArcRwLockReadGuardian::take(input.clone()) else {
            return Err("Could not aquire read lock on input storage".into());
        };

        self.read_guard = SendOption::new(Some(guard));

        self.view_keys.clear();

        for key in keys
        {
            self.view_keys.push(key);
        }

        Ok(())
    }

    fn create_write_view(&mut self, keys: Box<dyn Iterator<Item = Key>>) -> SimpleResult<()>
    {
        let Some(input) = &self.input_storage else {
            return Err("Input storage not set".into());
        };

        let Ok(guard) = ArcRwLockWriteGuardian::take(input.clone()) else {
            return Err("Could not aquire write lock on input storage".into());
        };

        self.write_guard = SendOption::new(Some(guard));

        self.view_keys.clear();

        for key in keys
        {
            self.view_keys.push(key);
        }

        Ok(())
    }
}

impl<InputStorage, Key, Item> Storage for KeyItemViewStorage<InputStorage, Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
    InputStorage: KeyItemStorage<Key = Key, Item = Item>,
{
    fn len(&self) -> usize
    {
        self.view_keys.len()
    }
}

impl<InputStorage, Key, Item> ClearableStorage for KeyItemViewStorage<InputStorage, Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
    InputStorage: KeyItemStorage<Key = Key, Item = Item>,
{
    // TODO: #MED We can't remove the keys as that is the responsibility of the 
    // ViewController so without a major change to the structure of the storage 
    // trait family the only sensible non panicking implementation is to reset 
    // each item in the view back to its default.  
    fn clear(&mut self)
    {
        todo!("Not implemented");
    }
}

// ---------------------------------------------------------------
// KeysToItemsIter
// ---------------------------------------------------------------

/// Converts an inner iterator of Keys into an iterator of keys and item
/// item references. The item is looked up from the passed in input storage.
///
/// # Safety
/// Implementation of [Iterator] for this type uses unsafe code. See
/// module documentation Safety section for details
pub struct KeysToItemsIter<'a, InputStorage, InnerIter, Item>
{
    input_storage: &'a InputStorage,
    mut_ptr_iter: InnerIter,
    phantom: PhantomData<Item>,
}

impl<'a, InputStorage, InnerIter, Item> KeysToItemsIter<'a, InputStorage, InnerIter, Item>
where
    Item: ItemTrait,
{
    pub fn new(
        input_storage: &'a InputStorage,
        iter: InnerIter,
    ) -> KeysToItemsIter<InputStorage, InnerIter, Item>
    {
        KeysToItemsIter {
            input_storage,
            mut_ptr_iter: iter,
            phantom: PhantomData,
        }
    }
}

impl<'a, InputStorage, KeysIter, Key, Item: 'a> Iterator
    for KeysToItemsIter<'a, InputStorage, KeysIter, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
    KeysIter: Iterator<Item = &'a Key>, // The inner iterator that iterates over the keys
    InputStorage: KeyItemStorage<Key = Key, Item = Item>,
{
    type Item = (Key, &'a Item);

    fn next(&mut self) -> Option<Self::Item>
    {
        let key: &Key = self.mut_ptr_iter.next()?;

        let Some(item) = self.input_storage.get(*key) else {
            return None
        };

        Some((*key, item))
    }
}

// ----------------------------------------------------------------------------------
// Helper Trait Implements
// ----------------------------------------------------------------------------------

impl<InputStorage, Key, Item> KeyTypeIdNoSelf for KeyItemViewStorage<InputStorage, Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
    InputStorage: KeyItemStorage<Key = Key, Item = Item>,
{
    fn key_type_id() -> std::any::TypeId
    {
        TypeId::of::<Key>()
    }
}

impl<InputStorage, Key, Item> ItemTypeIdNoSelf for KeyItemViewStorage<InputStorage, Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
    InputStorage: KeyItemStorage<Key = Key, Item = Item>,
{
    fn item_type_id() -> std::any::TypeId
    {
        TypeId::of::<Item>()
    }
}

#[cfg(test)]
mod tests
{
    use super::KeyItemViewStorage;
    use crate::{
        storage_traits::{KeyItemStorage, ViewStorageSetup, MutKeyItemStorage, Storage},
        Arw, storage_types::{VecStorage, SparseSetVecStorage},
    };
    use std::sync::{Arc, RwLock};

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    struct ComponentA(i32);

    #[test]
    fn vec_storage_test()
    {
        let mut storage: VecStorage<usize, ComponentA> = VecStorage::new();

        storage.insert_and_shift(0, ComponentA(0));
        storage.insert_and_shift(1, ComponentA(1));
        storage.insert_and_shift(2, ComponentA(2));
        storage.insert_and_shift(3, ComponentA(3));

        let input_storage_am: Arw<VecStorage<usize, ComponentA>> = Arc::new(RwLock::new(storage));

        // View ----------------------

        let mut view_storage: KeyItemViewStorage<VecStorage<usize, ComponentA>, usize, ComponentA> =
            KeyItemViewStorage::new();

        view_storage.set_input_storage(input_storage_am.clone());

        let vec = vec![2, 0, 1];
        view_storage.create_read_view(Box::new(vec.into_iter())).unwrap();

        // Confirm that iter works
        println!("view_storage.iter():");
        for item in view_storage.key_item_iter_static()
        {
            println!("{:?}", item);
        }

        assert_eq!(view_storage.get(0).unwrap(), &ComponentA(2));
        assert_eq!(view_storage.get(1).unwrap(), &ComponentA(0));
        assert_eq!(view_storage.get(2).unwrap(), &ComponentA(1));
    }

    #[test]
    fn sparse_storage_test()
    {
        let mut storage: SparseSetVecStorage<usize, ComponentA> = SparseSetVecStorage::new();

        storage.insert(0, ComponentA(0));
        storage.insert(1, ComponentA(1));
        storage.insert(2, ComponentA(2));
        storage.insert(3, ComponentA(3));

        let input_storage_am: Arw<SparseSetVecStorage<usize, ComponentA>> =
            Arc::new(RwLock::new(storage));

        // View ----------------------

        let mut view_storage: KeyItemViewStorage<
            SparseSetVecStorage<usize, ComponentA>,
            usize,
            ComponentA,
        > = KeyItemViewStorage::new();

        view_storage.set_input_storage(input_storage_am.clone());

        let vec = vec![2, 0, 1];
        view_storage.create_read_view(Box::new(vec.into_iter())).unwrap();

        // Confirm that iter works
        println!("view_storage.iter():");
        for item in view_storage.key_item_iter_static()
        {
            println!("{:?}", item);
        }

        assert_eq!(view_storage.get(0).unwrap(), &ComponentA(2));
        assert_eq!(view_storage.get(1).unwrap(), &ComponentA(0));
        assert_eq!(view_storage.get(2).unwrap(), &ComponentA(1));
    }

    #[test]
    fn read_write_test()
    {
        let mut storage: VecStorage<usize, ComponentA> = VecStorage::new();

        storage.insert_and_shift(0, ComponentA(0));
        storage.insert_and_shift(1, ComponentA(1));

        let input_storage_am: Arw<VecStorage<usize, ComponentA>> = Arc::new(RwLock::new(storage));

        let mut view_storage: KeyItemViewStorage<VecStorage<usize, ComponentA>, usize, ComponentA> =
            KeyItemViewStorage::new();

        view_storage.set_input_storage(input_storage_am.clone());

        let vec = vec![0, 1];
        view_storage.create_read_view(Box::new(vec.into_iter())).unwrap();

        assert_eq!(view_storage.len(), 2);

        // Reading the orig data will pass because you can alias immutable data
        {
            let read_guard = input_storage_am.try_read();
            assert!(read_guard.is_ok());

            // Confirm that storage items themselves can still be accessed immutably
            println!("input storage items:");
            let read_guard = read_guard.unwrap();
            let iter = read_guard.into_iter();
            for item in iter
            {
                println!("{:?}", item);
            }
        }

        {
            // Writing will fail as our read view is still active in RefViewStorage at this point
            let write_guard = input_storage_am.try_write();
            assert!(write_guard.is_err());
        }

        // However, if we clear our view data this will also clear out the read guard it has taken
        // out on the orig data
        view_storage.clear_view();

        {
            // And now we should be able to take out a write guard again because it will be the only
            // one
            let write_guard = input_storage_am.try_write();
            assert!(write_guard.is_ok());
        }
    }
}

use std::{
    any::TypeId,
    ops::{Deref, DerefMut},
    sync::{Arc, RwLock},
};

use crate::{
    casting,
    storage_traits::{
        ItemSliceStorage, ItemTrait, KeyItemStorage, KeyStorage, KeyTrait, MutKeyItemStorage,
        Storage, ViewStorageSetup, KeyTypeIdNoSelf, ItemTypeIdNoSelf,
    },
    Arw, SimpleResult, storage_types::VecStorage,
};

use super::{InputStorageLockStatus, ViewStorageController};

/// A Smart Pointer to any Storage type that implements [crate::storage_traits::Storage].
///
/// Allows basic meta data such as key and item type id to be checked at runtime without 
/// acquiring a lock to the inner guards. And holds a smart pointer to the base storage 
/// as well as any more specific storage type. These dual smart pointers aid in casting 
/// up and down the type / trait hierarchy.
///
/// This type also includes methods to cast to between different pointer types covering a range of
/// dynamic and static dispatch needs.
//
// ---------------------------------------------------------------------------------------------
//
// # Internal Design
//
// ## Use of Arc<RwLock<StorageType>>
//
// The storage fields within [StorageHandle] need to be behind a smart pointer of the form
// Arc<RwLock<T>> where T: ?Sized. This allows for containing a large range of types including:
// Sized Storage, dyn Storage, dyn StorageSuperTraitA, etc. The pointer is aliased to Arw<T>.
//
// Since we have a RwLock directly within Arc it is not possible to use Rusts built in casting
// functionality with [Any] to downcast to concrete types. There for we must use a small amount of
// unsafe code as well as the unstable ptr_metadata feature. To avoid unsafe and ptr_metadata we
// could consider the below alternatives.
//
// ### Alternatives
//
// - Move RwLock inside the Storage types so that safe `Any` style downcasting and third party
//   casting crates could be integrated. Crates such as downcast_rs, and cast_trait_object will work
//   with just safe code. However, my experiments have shown that moving a guard cell such as RwLock
//   inside your storage type forces you to implement a whole additional set of companion storage
//   traits. The first set of traits is to acquire a lock on the correct kind of storage trait object
//   and then once the lock is acquired you need to return another trait object that helps access the
//   contents of the returned guard. Given that there is a fair amount of storage traits already,
//   introducing a whole companion set seems to introduce more complexity and cognitive overhead for
//   users / maintainers than seems justifiable. so having a small amount of unsafe code to punch
//   through the RwLock on the outside of the storage type seems to be the way to go. note: The
//   experiments that revealed these issues can be seen and run under tests/experiments
// - We can try a direct cast between pointer types but to do this we need a guarantee about the
//   first word of a fat pointer always pointing to the data and a fat to thin cast always clipping
//   off the last words. As far as I have read I have not come across this as a statement of
//   stability so there is no way to jump off nightly until there is clarity on this. There is more
//   energy going into dyn Trait based work recently though so hopefully this is not far off to get
//   at least some minimal guarantee.
// - Introducing a Box such as Arc<Box<RwLock<StorageType>>> doesn't help as the major challenge is
//   that RwLock and its guards don't have CoerceUnsized (even in parking lot) and so there for we
//   still get blocked at that when attempting to cast.
// - Use a third party crate such as cast_trait_object for safe trait -> trait casting: These suffer
//   from the same problem if using only safe code and can't be used with RwLock directly inside an
//   Arc and so there for could only be useful if moving the RwLock into the storage types which has
//   the unfortunate consequence of doubling the number of traits needed.
// - There is a dyn upcasting coercion initiative for dyn upcasting which would mean that I could
//   upcast between trait objects without needing to first downcast then upcast again. Though I'm
//   uncertain if / how it may help with trait downcasting.
// [https://github.com/rust-lang/rust/issues/65991](Tracking Issue)
//
// ### Third party crates
//
// As discussed above, third party crates will also get stumped with safe downcasting an
// Arc<RwLock<dyn T>> and most likely have issues with safe cross casting too. Though I have not had
// time to confirm these assertions.
//
// Here are some crate options to consider to help with casting if any of the above thoughts change
// in future:
//
// - [https://crates.io/crates/cast_trait_object](cast_trait_object): Looks quite good
// - [https://crates.io/crates/trait_cast_rs](trait_cast_rs crate): Able to cast between any related
//   trait objects and also downcast to a concrete type with quite minimal boiler plate. However,
//   this crate depends on the unstable trait upcasting feature among several other unstable
//   features. So for the moment it seems too risky to depend on it.
// - There are other crates worth thinking about described in the Alternatives section of
//   cast_trait_object
//
// ## Extra Meta data
// - What if you need more meta data. A generic meta data type used to be a field in this pointer
// - however since the storage already comes with key and item type ID I removed it as it simplified 
//   all APIs considerably and if a user wishes to add their domain specific meta data they can either 
//   look it up in their own domain or create a domain specific pointer around this one with that 
//   meta data included
pub struct StorageHandle<S>
where
    S: Storage + ?Sized,
{
    // Until upcast coercion lands and also coercion / casting through a RwLock
    // there is no obvious straight forward way to upcast from an existing
    // storage supertrait object back to a storage trait object. So as a workaround
    // we just simply keep one around right from the start and return a reference to
    // it if its needed
    pub(super) base_storage: Arw<dyn Storage>,

    // A flexible internal pointer that can contain either a sized storage type or
    // a storage trait object such as a storage supertrait
    storage: Arw<S>,

    view_storage_controller: Option<ViewStorageController>,

    key_type_id: TypeId,
    item_type_id: TypeId,
}

impl<S> Clone for StorageHandle<S>
where
    S: Storage + ?Sized,
{
    fn clone(&self) -> Self
    {
        Self {
            base_storage: self.base_storage.clone(),
            storage: self.storage.clone(),
            view_storage_controller: self.view_storage_controller.clone(),
            key_type_id: self.key_type_id,
            item_type_id: self.item_type_id,
        }
    }
}

/// Casts [StorageHandle<SourceStorage>] to StorageHandle<TargetStorageTrait>
/// This produces cast functions with the same purpose as the lower level
/// [crate::casting] functions but introduces StorageHandle specifics into
/// the equation so that the casting can be done directly with [StorageHandle]
macro_rules! define_cast_storage_ptr_to_dyn_fn {

    ($fn_name:ident, $inner_fn_name:ident, $target_trait:ty) => {
        pub fn $fn_name<Key, Item>(self) -> SimpleResult<StorageHandle<$target_trait>>
        where
            Key: KeyTrait,
            Item: ItemTrait,
        {
            // Check that we are dealing with the same item type
            if TypeId::of::<Item>() != self.item_type_id()
            {
                return Err("Invalid cast due to unexpected item type id".into());
            }

            // Takes advantage of our casting modules lower level casting function
            let key_item_storage: Arc<RwLock<$target_trait>> =
                casting::$inner_fn_name::<S, Key, Item>(self.storage.clone())?;

            // And then we wrap that cast into a new appropriately typed
            // StorageHandle
            let storage_ptr = StorageHandle::<$target_trait> {
                base_storage: self.base_storage.clone(),
                storage: key_item_storage.clone(),
                view_storage_controller: self.view_storage_controller.clone(),
                key_type_id: self.key_type_id,
                item_type_id: self.item_type_id,
            };

            Ok(storage_ptr)
        }
    };
}

pub struct StorageHandleBuilder
{
    base_storage: Arw<dyn Storage>,
    // storage: Arw<S>,

    // Having these as TypeID instead of phantoms saves on some compile time
    key_type_id: TypeId,
    item_type_id: TypeId,

    // Items below are optionally built
    // --------------------------------

    view_storage_controller: Option<ViewStorageController>,
}

impl StorageHandleBuilder
{
    pub fn new<S>(storage: S) -> Self
    where
        S: Storage + Into<Arw< dyn Storage>> + KeyTypeIdNoSelf + ItemTypeIdNoSelf,
    {
        Self {
            base_storage: storage.into(),
            key_type_id: S::key_type_id(),
            item_type_id: S::item_type_id(),
            view_storage_controller: None,
        }
    }

    pub fn add_view_controller(&mut self) -> &mut Self
    {
        self.view_storage_controller = Some(ViewStorageController::new(
            self.base_storage.clone(),
            Arc::new(RwLock::new(InputStorageLockStatus::None)),
        ));

        self
    }

    pub fn build(self) -> StorageHandle<dyn Storage>
    {
        StorageHandle::<dyn Storage> {
            base_storage: self.base_storage.clone(),
            storage: self.base_storage.clone(),
            view_storage_controller: self.view_storage_controller,
            key_type_id: self.key_type_id,
            item_type_id: self.item_type_id,
        }
    }
}

/// Convenience function to create a [StorageHandleBuilder] without
/// needing to refer to the longer [StorageHandleBuilder] name
pub fn builder<S>(storage: S) -> StorageHandleBuilder
where
    S: Storage + Into<Arw< dyn Storage>> + KeyTypeIdNoSelf + ItemTypeIdNoSelf,
{
    StorageHandleBuilder::new::<S>(storage)
}

impl<S> StorageHandle<S>
where
    S: Storage + ?Sized,
{
    pub fn new(
        storage: Arw<S>,
        base_storage: Arw<dyn Storage>,
        key_type_id: TypeId,
        item_type_id: TypeId,
    ) -> Self
    {
        Self {
            base_storage,
            storage,
            view_storage_controller: None,
            key_type_id,
            item_type_id,
        }
    }

    // TODO: #LOW Use the builder pattern instead to build with a view
    pub fn new_with_view_controller(
        storage: Arw<S>,
        base_storage: Arw<dyn Storage>,
        key_type_id: TypeId,
        item_type_id: TypeId,
    ) -> Self
    {
        let view_controller: Option<ViewStorageController> = Some(ViewStorageController::new(
            base_storage.clone(),
            Arc::new(RwLock::new(InputStorageLockStatus::None)),
        ));

        Self {
            base_storage,
            storage,
            view_storage_controller: view_controller,
            key_type_id,
            item_type_id,
        }
    }

    pub fn view_storage_controller(&self) -> Option<&ViewStorageController>
    {
        self.view_storage_controller.as_ref()
    }

    pub fn view_storage_controller_mut(&mut self) -> Option<&mut ViewStorageController>
    {
        self.view_storage_controller.as_mut()
    }

    pub fn key_type_id(&self) -> TypeId
    {
        self.key_type_id
    }

    pub fn item_type_id(&self) -> TypeId
    {
        self.item_type_id
    }

    // Relevance of [ViewStorageController] in try_read and try_write blocks
    // ----------------------------------------------------------------------
    // The try_read and try_write methods employ an important guard
    // condition for cases where a StorageHandle has a view controller set. Basically
    // if one has been set, then we also check that the ViewStorage has a created
    // view and if not return early with an error. This check is very powerful because it
    // blocks users from interacting with the ViewStorage API prior to
    // its view being created / setup correctly.

    pub fn try_read(&self) -> SimpleResult<impl Deref<Target = S> + '_>
    {
        // If there is a view controller, ensure that the view has been created
        if let Some(view_controller) = &self.view_storage_controller
        {
            if view_controller.status()? == InputStorageLockStatus::None
            {
                return Err("Cannot aquire a read lock on the ViewStorage as ViewController::status == None. A View must be created first using the ViewController".into());
            }
        }

        if let Ok(guard) = self.storage.try_read()
        {
            Ok(guard)
        }
        else
        {
            Err("Failed to aquire read guard".into())
        }
    }

    pub fn try_write(&self) -> SimpleResult<impl DerefMut<Target = S> + '_>
    {
        // If there is a view controller, ensure that the view has been created
        if let Some(view_controller) = &self.view_storage_controller
        {
            if view_controller.status()? == InputStorageLockStatus::None
            {
                return Err("Cannot aquire a write lock on the ViewStorage as ViewController::status == None. A View must be created first using the ViewController".into());
            }
        }

        if let Ok(guard) = self.storage.try_write()
        {
            Ok(guard)
        }
        else
        {
            Err("Failed to aquire write guard".into())
        }
    }

    // ----------------------------------------------------------
    // Casting
    // ----------------------------------------------------------

    // To Trait object casting

    define_cast_storage_ptr_to_dyn_fn!(
        cast_to_key_storage,
        cast_to_key_storage,
        dyn KeyStorage<Key = Key>
    );
    define_cast_storage_ptr_to_dyn_fn!(
        cast_to_getitem_storage,
        cast_to_dyn_getkeyitemstorage,
        dyn KeyItemStorage<Key = Key, Item = Item>
    );
    define_cast_storage_ptr_to_dyn_fn!(
        cast_to_keyitemview_storage,
        cast_to_dyn_getkeyitemviewstorage,
        dyn ViewStorageSetup<Key = Key>
    );
    define_cast_storage_ptr_to_dyn_fn!(
        cast_to_mut_getitem_storage,
        cast_to_dyn_mutitemstorage,
        dyn MutKeyItemStorage<Key = Key, Item = Item>
    );
    define_cast_storage_ptr_to_dyn_fn!(
        cast_to_slice_storage,
        cast_to_dyn_sliceitemstorage,
        dyn ItemSliceStorage<Item = Item>
    );

    /// Downcast to TargetType where Target type is Sized
    pub fn cast_to_sized_storage<TargetType>(self) -> SimpleResult<StorageHandle<TargetType>>
    where
        TargetType: Storage + Sized,
    {
        let target_type: Arc<RwLock<TargetType>> =
            casting::dyn_storage_into_sized::<S, TargetType>(self.storage.clone())?;

        let storage_ptr = StorageHandle::<TargetType> {
            base_storage: self.base_storage.clone(),
            storage: target_type,
            view_storage_controller: self.view_storage_controller.clone(),
            key_type_id: self.key_type_id,
            item_type_id: self.item_type_id,
        };

        Ok(storage_ptr)
    }
}

/// Convert the [StorageHandle] into a base storage pointer
//
// -------------------------------------------------------------------------------------------------
// # Internal Design
//
// * In current rust this kind of inter trait object upcast (especially when a RwLock is involved in
//   a smart pointer) has no decent or built in solution. There for this function exploits the fact
//   that we keep a base dyn Storage pointer around as a backup within the [StorageHandle] and can
//   there for get another [StorageHandle] using that base trait again.
//
// * This is a free standing function because when I try to make it method inside [StorageHandle] rust
//   complains about certain trait requirements not being met.
pub fn storage_ptr_into_base<StorageType>(
    storage_ptr: StorageHandle<StorageType>,
) -> SimpleResult<StorageHandle<dyn Storage>>
where
    StorageType: Storage + ?Sized,
{
    let storage_ptr: StorageHandle<dyn Storage> = StorageHandle::new(
        storage_ptr.base_storage.clone(),
        storage_ptr.base_storage.clone(),
        storage_ptr.key_type_id,
        storage_ptr.item_type_id,
    );

    Ok(storage_ptr)
}

impl <Key, Item> From<VecStorage<Key, Item>> for Arw<dyn Storage> 
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn from(value: VecStorage<Key, Item>) -> Self {

        let storage = Arc::new(RwLock::new(value));
        let storage: Arw<dyn Storage> = storage;
        storage
    }
}

#[cfg(test)]
pub mod tests
{
    use std::{
        any::TypeId,
        sync::{Arc, RwLock},
    };

    use crate::{
        // storage_ptr::builder_from_arw,
        storage_types::VecStorage,
        storage_traits::{ItemSliceStorage, KeyItemStorage, Storage},
        Arw, storage_handle::builder,
    };

    use super::{storage_ptr_into_base, StorageHandle};

    #[test]
    fn cast_to_sized_storage_test()
    {
        let storage: VecStorage<usize, i32> = VecStorage::new_from_iter(vec![1, 2, 3]);

        let storage_ptr = builder(storage).build();

        let storage_ptr_concrete: StorageHandle<VecStorage<usize, i32>> =
            storage_ptr.cast_to_sized_storage().unwrap();

        let guard = storage_ptr_concrete.try_read().unwrap();

        let mut sum: i32 = 0;
        for i in guard.into_iter()
        {
            sum += i;
        }

        assert_eq!(sum, 6);
    }

    #[test]
    fn cast_to_itemslice_storage_test()
    {
        let storage: VecStorage<usize, i32> = VecStorage::new_from_iter(vec![1, 2, 3]);

        let storage = Arc::new(RwLock::new(storage));
        let storage: Arw<dyn Storage> = storage;

        let storage_ptr: StorageHandle<dyn Storage> = StorageHandle::new(
            storage.clone(),
            storage,
            TypeId::of::<usize>(),
            TypeId::of::<i32>(),
        );

        let storage_ptr: StorageHandle<dyn ItemSliceStorage<Item = i32>> =
            storage_ptr.cast_to_slice_storage::<usize, i32>().unwrap();

        let guard = storage_ptr.try_read().unwrap();

        let mut sum: i32 = 0;
        for i in guard.as_item_slice()
        {
            sum += i;
        }

        assert_eq!(sum, 6);
    }

    /// Test that a pointer can be cast several times to any type that participates in the storage
    /// trait family of traits. This shows how flexible the pointer really is.
    #[test]
    fn cast_journey()
    {
        // Create a concrete storage type
        let storage: VecStorage<usize, i32> = VecStorage::new_from_iter(vec![1, 2, 3]);

        // Put it into a handle to a dyn Storage trait object - the root trait of all storage traits.
        let storage_handle: StorageHandle<dyn Storage> = builder(storage).build();

        // The handle can be used to freely cast between any of the supported supertraits
        // of the Storage trait via the handles cast methods.

        // Inter trait object cast to a handle to dyn ItemSliceStorage
        let slice_handle: StorageHandle<dyn ItemSliceStorage<Item = i32>> = storage_handle
            .cast_to_slice_storage::<usize, i32>()
            .unwrap();

        // Access the trait ItemSliceStorage trait objects items
        {
            let guard = slice_handle.try_read().unwrap();
            let slice: &[i32] = guard.as_item_slice();

            assert_eq!(slice[0], 1);
            assert_eq!(slice[1], 2);
        }

        // Inter trait object cast to a handle to a dyn KeyItemStorage
        let key_item_handle: StorageHandle<dyn KeyItemStorage<Key = usize, Item = i32>> =
            slice_handle.cast_to_getitem_storage().unwrap();

        // Access the trait KeyItemStorage trait objects items
        {
            // Get a guard to the dyn KeyItemStorage StorageHandle
            let guard = key_item_handle.try_read().unwrap();

            assert_eq!(guard.get(0).unwrap(), &1);
            assert_eq!(guard.get(1).unwrap(), &2);
        }

        // Iterate using static dispatch
        {
            // Cast to a concrete ptr
            let storage_ptr: StorageHandle<VecStorage<usize, i32>> =
                key_item_handle.cast_to_sized_storage().unwrap();

            let guard = storage_ptr.try_read().unwrap();

            let mut sum: i32 = 0;
            for i in guard.into_iter()
            {
                sum += i;
            }

            assert_eq!(sum, 6);
        }
    }

    #[test]
    fn into_base_storage_test()
    {
        let storage: VecStorage<usize, i32> = VecStorage::new_from_iter(vec![1, 2, 3]);
        let storage = Arc::new(RwLock::new(storage));
        let storage: Arw<dyn Storage> = storage;

        let storage_ptr: StorageHandle<dyn Storage> = StorageHandle::new(
            storage.clone(),
            storage,
            TypeId::of::<usize>(),
            TypeId::of::<i32>(),
        );

        let storage_ptr: StorageHandle<dyn KeyItemStorage<Key = usize, Item = i32>> =
            storage_ptr.cast_to_getitem_storage().unwrap();

        let _ = storage_ptr_into_base(storage_ptr);
    }
}

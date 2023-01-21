//! Provides functions for casting between rust std lib pointer wrapped storage types.
//!
//! The cast methods deal with a range of smart pointer types covering both sized and unsized
//! use cases. For example:
//! - [`Arw<dyn Storage>`]
//! - [Arw]<dyn '[Storage] Supertrait'>
//! - [`Arw<Storage>`] where [Storage]: [Sized]
//!
//! Arw is a type alias for [`Arc<RwLock<T>>`]
//!
//! This module deals only with Arw pointers however there is another higher level
//! pointer type that wraps that one and adds that its own meta data for runtime type
//! inspection. See: [crate::storage_handle::StorageHandle] for details.

// # Internal Design
//
// ## Cast to dyn trait
//
// All cast_to_dyn_<trait> functions use this pattern to cast:
// Casting is achieved by first trying to downcast to a valid concrete type
// followed by implicit coerce unsize based casting thats currently
// built into rust.
//
// # Limitations
// The cast functions only work with the base storage trait: Arw<dyn Storage>, because upcast
// coercion has not been completed in rust. An attempted workaround using generics and the Unsize
// trait in the casting functions didn't get around the problem. For example:
// If using [Arw<SourceStorageType>] where SourceStorageType: Storage + Unsized<dyn Storage> it still
// won't accept supertraits of Storage like Arw<dyn [KeyItemStorage<Key=Key, Item=Item>]>

use std::{
    any::{type_name, TypeId},
    ptr::Pointee,
    sync::{Arc, RwLock},
};

use crate::{

    storage_traits::{
        ItemSliceStorage, ItemTrait, KeyItemStorage, KeyStorage, KeyTrait,
        MutItemSliceStorage, MutKeyItemStorage, Storage,
        ViewStorageSetup,
    },
    storage_types::{
        HashMapStorage, VecStorage, KeyItemViewStorage, SparseSetVecStorage, ValStorage,
    },
    Arw, SimpleResult,
};

/// Casts [Arw<SourceStorage>] to [Arw]<dyn [TargetStorageTrait]>
//
// # Internal Design
//
// The Item type needs to be supplied even though some target traits don't have an 
// associated Item type. The reason for this is that we are performing casting
// by first downcasting and then upcasting. And to downcast we need the full concrete type
// signature which of course involves keys and items as all of our types use keys and items
// even ValStorage for compatibility reasons.
macro_rules! define_cast_to_dyn_fn {

    ($fn_name:ident, $target_trait:ty, [$($related_type:ty),*]) => {

        pub fn $fn_name<SourceStorage, Key, Item>(
            source_storage: Arw<SourceStorage>,
        ) -> SimpleResult<Arw<$target_trait>>
        where
            SourceStorage: Storage + ?Sized,
            Key: KeyTrait,
            Item: ItemTrait,
        {
            $(
                if let Ok(target_type) =
                    dyn_storage_into_sized::<SourceStorage, $related_type>(source_storage.clone())
                {
                    let storage: Arw<$target_trait> = target_type;
                    return Ok(storage);
                };
            )*

            Err(format!(
                "Invalid cast from '{}' into '{}'",
                type_name::<SourceStorage>(),
                type_name::<$target_trait>()
            ))
        }

    };
}

/// Cast [`Arw<SourceStorage>`] to [`Arw<TargetStorageType>`]
// ------------------------------------------------------
//
// # Internal Design
//
// ## Safety
// A cast from Arc<dyn Storage> -> Arc<StorageType> is possible in safe rust. But in our case,
// the desired cast is from Arc<RwLock<dyn Storage>> -> Arc<RwLock<TargetStorageType>>,
// and since the [RwLock] sits between the Arc and the dyn Storage, rust cant downcast to a
// concrete type using only safe code.
//
// So the solution used here is to use some unsafe code and the ptr_metadata Nightly feature.
// Details on the feature:
// Doc: https://doc.rust-lang.org/nightly/std/primitive.pointer.html#method.to_raw_parts
// Issue: https://github.com/rust-lang/rust/issues/81513
//
// ## Warning about having downcast_rs::Downcast in scope
//
// If you have ``` use downcast_rs::Downcast; ``` in scope for this module
// the compiler will report lifetime errors. This is because the Any and or type_id
// functionality from Downcast has differences with regular any.
//
// ## Alternatives to using unsafe code and the ptr_metadata feature
// - Introduce a Box so that the full concrete type becomes Arc<Box<RwLock<StorageType>>> and the
//   dyn type might be Arc<Box<dyn Storage>>. Basically the Arc<Box should make it compatible with
//   the Any downcast methods. Unfortunately after testing this Arc<Box<RwLock<StorageType>>> can't
//   be coerced into either Arc<Box<dyn Storage>> or Arc<dyn Storage> most likely because RwLock has
//   no Coerce unsized support.
// - Transmute to a raw internal only and unstable TraitObject. However, it seems that the raw
//   feature and access to the TraitObject struct has been removed and instead ptr_meta data is
//   being promoted instead. The advantage with ptr_meta data is that its trying to actually come up
//   with a stable API for this stuff.
// - Directly cast from the fat to a thin pointer and hope that the pointer to the erased type is
//   the part that is kept (which might change in future releases).
// - Wait for improvements to trait objects, trait object casting, etc. There is a lot of incomplete
//   work going on in this space though its taking time.
pub fn dyn_storage_into_sized<SourceStorage, TargetStorageType>(
    source_storage: Arw<SourceStorage>,
) -> SimpleResult<Arw<TargetStorageType>>
where
    SourceStorage: Storage + ?Sized,
    TargetStorageType: Storage,
{
    // Safety: Before doing any pointer work - confirm that the source storage trait object
    // points to type data that is of the expected type
    {
        // TODO: #HIGH return an error instead of unwrapping
        let borrow = source_storage.try_read().unwrap();

        // To avoid getting the type id of the RefCell or the RC,
        // as_any() is required to get the correct &Any object to
        // perform the type_id call on.
        let any = borrow.as_any();

        if TypeId::of::<TargetStorageType>() != any.type_id()
        {
            return Err(format!(
                "Invalid cast to sized from '{}' into '{}'",
                type_name::<SourceStorage>(),
                type_name::<TargetStorageType>()
            ));
        }
    }

    let raw_ptr: *const RwLock<SourceStorage> = Arc::into_raw(source_storage);

    let (type_erased_ptr, _): (*const (), <RwLock<SourceStorage> as Pointee>::Metadata) =
        raw_ptr.to_raw_parts();

    let typed_data_ptr = type_erased_ptr as *const RwLock<TargetStorageType>;
    let arc = unsafe { Arc::from_raw(typed_data_ptr) };

    Ok(arc)
}

// Cast [Arw<SourceStorage>] to [Arw]<dyn [KeyItemStorage<Key=Key, Item=Item>]>
#[rustfmt::skip]
define_cast_to_dyn_fn!( 
    cast_to_dyn_getkeyitemstorage,              // fn name
    dyn KeyItemStorage<Key = Key, Item = Item>, // target trait

    // Storage types that can be cast to the target trait
    [
        VecStorage<Key, Item>,
        SparseSetVecStorage<Key, Item>,
        HashMapStorage<Key, Item>,
        ValStorage<Key, Item>,

        // Repetition of above with views
        KeyItemViewStorage<VecStorage<Key, Item>, Key, Item>,
        KeyItemViewStorage<SparseSetVecStorage<Key, Item>, Key, Item>,
        KeyItemViewStorage<HashMapStorage<Key, Item>, Key, Item>,
        KeyItemViewStorage<ValStorage<Key, Item>, Key, Item>
    ]
);

// Cast [Arw<SourceStorage>] to [Arw]<dyn [MutKeyItemStorage<Key=Key, Item=Item>]>
#[rustfmt::skip]
define_cast_to_dyn_fn!( 
    cast_to_dyn_mutitemstorage,
    dyn MutKeyItemStorage<Key = Key, Item = Item>, // target trait

    // Storage types that can be cast to the target trait
    [
        VecStorage<Key, Item>,
        SparseSetVecStorage<Key, Item>,
        HashMapStorage<Key, Item>,

        // TODO: These don't have an implementation of GetItemMut yet or at least they cause compiler errors 
        // when uncommented - needs investigation
        // ValStorage<Key, Item>,
        // KeyItemViewStorage<ValStorage<Key, Item>, Key, Item>

        // Repetition of above with views
        KeyItemViewStorage<VecStorage<Key, Item>, Key, Item>,
        KeyItemViewStorage<SparseSetVecStorage<Key, Item>, Key, Item>,
        KeyItemViewStorage<HashMapStorage<Key, Item>, Key, Item>
    ]
);

// Cast [Arw<SourceStorage>] to [Arw]<dyn [KeyStorage<Key=Key>]>
#[rustfmt::skip]
define_cast_to_dyn_fn!( 
    cast_to_key_storage,       // fn name
    dyn KeyStorage<Key = Key>, // target trait

    // Storage types that can be cast to the target trait
    [
        VecStorage<Key, Item>,
        SparseSetVecStorage<Key, Item>,
        HashMapStorage<Key, Item>,
        ValStorage<Key, Item>,

        // Repetition of above with views
        KeyItemViewStorage<VecStorage<Key, Item>, Key, Item>,
        KeyItemViewStorage<SparseSetVecStorage<Key, Item>, Key, Item>,
        KeyItemViewStorage<HashMapStorage<Key, Item>, Key, Item>,
        KeyItemViewStorage<ValStorage<Key, Item>, Key, Item>
    ]
);

// Cast [Arw<SourceStorage>] to [Arw]<dyn [ViewStorageSetup<Key=Key]>
#[rustfmt::skip]
define_cast_to_dyn_fn!( 
    cast_to_dyn_getkeyitemviewstorage, // fn name
    dyn ViewStorageSetup<Key = Key>,   // target trait

    // Storage types that can be cast to the target trait
    [
        KeyItemViewStorage<VecStorage<Key, Item>, Key, Item>,
        KeyItemViewStorage<SparseSetVecStorage<Key, Item>, Key, Item>,
        KeyItemViewStorage<HashMapStorage<Key, Item>, Key, Item>,
        KeyItemViewStorage<ValStorage<Key, Item>, Key, Item>
    ]
);

// Cast [Arw<SourceStorage>] to [Arw]<dyn [ItemSliceStorage<Item=Item]>
#[rustfmt::skip]
define_cast_to_dyn_fn!( 
    cast_to_dyn_sliceitemstorage,        // fn name
    dyn ItemSliceStorage<Item = Item>,   // target trait

    // Storage types that can be cast to the target trait
    [
        VecStorage<Key, Item>,
        SparseSetVecStorage<Key, Item>,
        ValStorage<Key, Item>

        // ViewStorage types are excluded as there is no contiguous Item data that they can 
        // return due to these kinds of views being able to filter using sparse items locations
    ]
);

// Cast [Arw<SourceStorage>] to [Arw]<dyn [MutItemSliceStorage<Item=Item]>
#[rustfmt::skip]
define_cast_to_dyn_fn!( 
    cast_to_dyn_mutsliceitemstorage,        // fn name
    dyn MutItemSliceStorage<Item = Item>,   // target trait

    // Storage types that can be cast to the target trait
    [
        VecStorage<Key, Item>,
        SparseSetVecStorage<Key, Item>,
        ValStorage<Key, Item>

        // ViewStorage types are excluded as there is no contiguous Item data that they can 
        // return due to these kinds of views being able to filter using sparse items locations
    ]
);

/// TODO: #LOW Consider moving some of these into doc tests where feasible
#[cfg(test)]
mod tests
{
    use std::sync::{Arc, RwLock};

    use crate::{
        casting::{cast_to_dyn_sliceitemstorage, dyn_storage_into_sized},
        storage_types::{VecStorage, SparseSetVecStorage},
        Arw, Rw, storage_traits::{Storage, KeyItemStorage, ItemSliceStorage, MutKeyItemStorage},
    };

    use crate::casting::cast_to_dyn_getkeyitemstorage;

    /// Should panic when attempting to use a key that cannot be converted to an index
    /// This behavior is very important to letting a user know early on that they
    /// cant use certain key types with certain storages such as this one. And furthermore
    /// this could not be enforced at compile time via as we would have needed two different
    /// traits for keys which then causes issues with our base trait to child trait casting
    /// functions.
    #[test]
    #[should_panic]
    fn key_supports_index_test()
    {
        let vec_storage: VecStorage<u128, i32> = VecStorage::new_from_iter(vec![1, 2, 3]);

        // Prepare the source
        let storage: Arw<VecStorage<u128, i32>> = Arc::new(RwLock::new(vec_storage.clone()));
        let storage: Arw<dyn Storage> = storage;

        let _: Arw<dyn KeyItemStorage<Key = u128, Item = i32>> =
            cast_to_dyn_getkeyitemstorage(storage).unwrap();
    }

    /// Upcast a variety of storage types to the Storage trait and a range of Storage supertraits
    /// These are all able to be done thanks to rusts built in upcast coercion from a concrete type
    /// to an unsized type. Even if within smart pointers such as Arc<RwLock<dyn Storage>>
    /// Downcasting is less trivial and requires custom code which can be seen in the other tests
    #[test]
    fn concrete_to_dyn_trait_implicit_coercions_test()
    {
        let vec_storage: VecStorage<usize, i32> = VecStorage::new_from_iter(vec![1, 2, 3]);

        let mut sparse_storage: SparseSetVecStorage<usize, i32> = SparseSetVecStorage::new();
        sparse_storage.insert(0, 0);
        sparse_storage.insert(1, 1);
        sparse_storage.insert(2, 2);

        // Simple cast from concrete ref to dyn ref
        {
            let _: &dyn Storage = &vec_storage;
        }

        // ------------------------------------------------------------------------
        // RwLock<StorageType<i32>> -> RwLock<dyn <Storage<Item = i32>>
        // ------------------------------------------------------------------------

        // RwLock<VecStorage<usize, i32>> -> &RwLock<dyn SliceAccess<Item = i32>>
        {
            let vec_storage_rw: Rw<VecStorage<usize, i32>> = Rw::new(vec_storage.clone());
            let val: &Rw<dyn ItemSliceStorage<Item = i32>> = &vec_storage_rw;
            let read_guard = val.try_read().unwrap();

            assert_eq!(read_guard.len(), 3);

            let slice = read_guard.as_item_slice();
            for item in slice
            {
                dbg!(item);
            }
        }

        // -------------------------------------------------------------------------
        // Arc<RwLock<StorageType<i32>>> -> Arc<RwLock<dyn <Storage<Item = i32>>>
        // -------------------------------------------------------------------------

        // Arc<RwLock<VecStorage<usize, i32>>> -> Arc<RwLock<dyn SliceAccess<Item = i32>>>
        {
            let storage: Arw<VecStorage<usize, i32>> = Arc::new(Rw::new(vec_storage.clone()));
            let storage: Arw<dyn ItemSliceStorage<Item = i32>> = storage;
            let guard = storage.try_read().unwrap();

            assert_eq!(guard.len(), 3);

            let slice = guard.as_item_slice();
            for item in slice
            {
                dbg!(item);
            }
        }

        // Arc<RwLock<SparseSetVecStorage<i32>>> -> Arc<RwLock<dyn SliceAccess<Item = i32>>>
        {
            let storage: Arw<SparseSetVecStorage<usize, i32>> =
                Arc::new(Rw::new(sparse_storage.clone()));
            let storage: Arw<dyn ItemSliceStorage<Item = i32>> = storage;
            let guard = storage.try_read().unwrap();

            assert_eq!(guard.len(), 3);

            let slice = guard.as_item_slice();
            for item in slice
            {
                dbg!(item);
            }
        }
    }

    #[test]
    fn simple_downcast_to_sized()
    {
        let mut vec_storage: VecStorage<usize, i32> = VecStorage::new_from_iter(vec![1, 2, 3]);

        // Downcast from dyn Storage -> VecStorage
        {
            let storage: &mut dyn Storage = &mut vec_storage;
            let vec_storage = storage.downcast_mut::<VecStorage<usize, i32>>().unwrap();
            vec_storage.set(0, 0);
        }

        // Arc<dyn Storage> -> Arc<VecStorage<usize, i32>>
        {
            // prepare the source
            let storage: Arc<VecStorage<usize, i32>> = Arc::new(vec_storage.clone());
            let storage: Arc<dyn Storage> = storage;

            // cast
            let storage = storage
                .downcast_arc::<VecStorage<usize, i32>>()
                .map_err(|_| "Cast error")
                .unwrap();

            assert_eq!(storage.len(), 3);
        }
    }

    /// Test a cast from Arw<dyn Storage> -> Arw<VecStorage<usize, i32>>
    #[test]
    fn dyn_storage_into_sized_test()
    {
        let vec_storage: VecStorage<usize, i32> = VecStorage::new_from_iter(vec![1, 2, 3]);

        {
            // Prepare the source
            let storage: Arw<VecStorage<usize, i32>> = Arc::new(RwLock::new(vec_storage.clone()));
            let storage: Arw<dyn Storage> = storage;

            let storage: Arw<VecStorage<usize, i32>> =
                dyn_storage_into_sized::<dyn Storage, VecStorage<usize, i32>>(storage).unwrap();

            let guard: std::sync::RwLockReadGuard<VecStorage<usize, i32>> =
                storage.try_read().unwrap();
            assert_eq!(guard.len(), 3);
        }
    }

    #[test]
    fn cast_to_dyn_itemslice_test()
    {
        let vec_storage: VecStorage<usize, i32> = VecStorage::new_from_iter(vec![1, 2, 3]);

        // Prepare the source
        let storage: Arw<VecStorage<usize, i32>> = Arc::new(RwLock::new(vec_storage.clone()));
        let storage: Arw<dyn Storage> = storage;

        // Cast from A
        let slice_storage: Arw<dyn ItemSliceStorage<Item = i32>> =
            cast_to_dyn_sliceitemstorage::<dyn Storage, usize, i32>(storage).unwrap();

        let guard = slice_storage.try_read().unwrap();
        assert_eq!(guard.as_item_slice().len(), 3);
    }

    /// A trait obj to trait object cast: [Arw<dyn Storage>] -> [Arw<dyn ItemSliceStorage<Item =
    /// i32>>]
    #[test]
    fn cast_to_dyn_getkeyitem_test()
    {
        let vec_storage: VecStorage<usize, i32> = VecStorage::new_from_iter(vec![1, 2, 3]);

        // Prepare the source
        let storage: Arw<VecStorage<usize, i32>> = Arc::new(RwLock::new(vec_storage.clone()));
        let storage: Arw<dyn Storage> = storage;

        // Cast
        let slice_storage: Arw<dyn KeyItemStorage<Key = usize, Item = i32>> =
            cast_to_dyn_getkeyitemstorage::<dyn Storage, usize, i32>(storage).unwrap();

        let guard = slice_storage.try_read().unwrap();
        assert_eq!(guard.get(0).unwrap(), &1);
    }
}

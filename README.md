Flex Storage empowers API's to be more abstract over data by focusing on shared traits and flexible casting. It provides
Storage Handles to point to either concrete Storage types or trait objects depending on if dynamic or static dispatch is
needed, and casting infrastructure to perform inter trait-object casts, trait-object to type casts and type un-sizing to
trait-objects.

The crate was made to support a primary use case of dataflow processing within a node based visual programming
environment where the intention is to be able to use storage types as inputs for processing which can be switched at
runtime with interchangeable storage handles. Such a high degree of runtime flexibility comes with some added API
complexity as well as performance considerations so consider a simpler static dispatch oriented workflow if most of your
storage design can be determined at compile time.

# Features

* Support for both dynamic and static dispatch though though dynamic dispatch workflows have had more work.
* Flexible casting between any type or trait object within the Storage trait family.
* Can be used to hold multiple handles to the same storage where each pointer can represent the storage as a different
  trait object or concrete type to fit the use case. This is ideal for graph based data processing.
* Primary use case is multithreaded so all storage types and handles are Send + Sync and use Arc<RwLock<StorageType>>
  internally within StorageHandles.
* NIGHTLY + UNSAFE: The library uses a single unsafe statement to perform casting that involves an
  Arc<RwLock<StorageType>> and also uses a nightly only feature called ptr_metadata to help promote safety in this
  unsafe cast.

# Example

The following demonstrates how its possible to easily cast between different storage trait objects with the help of
StorageHandles and also how the storage types items can be accessed via those different trait objects with dynamic
dispatch and also static dispatch if needed.

```rust
use ngenate_flex_storage::{
    storage_handle::{handle, StorageHandle},
    storage_traits::{ItemSliceStorage, KeyItemStorage, Storage}, storage_types::VecStorage,
};

fn main()
{
    // Create a concrete storage type
    let storage: VecStorage<usize, i32> = VecStorage::new_from_iter(vec![1, 2, 3]);

    // Put it into a handle to a dyn Storage trait object - the root trait of all storage traits.
    let storage_handle: StorageHandle<dyn Storage> = handle::builder(storage).build();

    // The handle can be used to freely cast between any of the supported supertraits
    // of the Storage trait via the handles cast methods.

    // Inter trait object cast to a handle to dyn ItemSliceStorage
    let slice_handle: StorageHandle<dyn ItemSliceStorage<Item = i32>> = storage_handle
        .cast_to_slice_storage::<usize, i32>()
        .unwrap();

    println!("Access the trait ItemSliceStorage trait objects items");
    {
        let guard = slice_handle.try_read().unwrap();
        let slice: &[i32] = guard.as_item_slice();

        dbg!(slice[0]);
        dbg!(slice[1]);
    }

    // Inter trait object cast to a handle to a dyn KeyItemStorage
    let key_item_handle: StorageHandle<dyn KeyItemStorage<Key = usize, Item = i32>> =
        slice_handle.cast_to_getitem_storage().unwrap();

    println!("Access the trait KeyItemStorage trait objects items");
    {
        // Get a guard to the dyn KeyItemStorage StorageHandle
        let guard = key_item_handle.try_read().unwrap();

        dbg!(guard.get(0).unwrap());
        dbg!(guard.get(1).unwrap());
    }

    // It's possible to switch back to a static dispatch handle
    // at any time.

    println!("Iterate using static dispatch");
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

        dbg!(sum);
    }
}
```
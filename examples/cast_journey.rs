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

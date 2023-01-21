use std::{
    any::TypeId,
    sync::{Arc, RwLock},
};

use ngenate_flex_storage::{
    storage_handle::{StorageHandle, ViewStorageController},
    storage_types::{KeyItemViewStorage, VecStorage}, storage_traits::{Storage, KeyItemStorage, MutKeyItemStorage},
};

// ViewStorage has its own unit tests, however this is an integration test between
// ViewStorage StorageHandle and the ViewGateway
#[test]
fn view_storage_read_test()
{
    // Create the source storage
    let input_storage_ptr: StorageHandle<dyn Storage> = {
        let storage: VecStorage<usize, i32> = VecStorage::new_from_iter(vec![0, 1, 2, 3, 4]);
        let storage = Arc::new(RwLock::new(storage));

        let storage_ptr: StorageHandle<dyn Storage> = StorageHandle::new(
            storage.clone(),
            storage,
            TypeId::of::<usize>(),
            TypeId::of::<i32>(),
        );

        storage_ptr
    };

    // Create the view storage (Not the actual view yet)
    let mut view_storage_ptr_dyn_storage: StorageHandle<dyn Storage> = {
        let storage: KeyItemViewStorage<VecStorage<usize, i32>, usize, i32> = KeyItemViewStorage::new();
        let storage = Arc::new(RwLock::new(storage));

        let storage_ptr: StorageHandle<dyn Storage> = StorageHandle::new_with_view_controller(
            storage.clone(),
            storage,
            TypeId::of::<usize>(),
            TypeId::of::<i32>(),
        );

        storage_ptr
    };

    // Create a view using the supplied view keys
    {
        let view_controller: &mut ViewStorageController =
            view_storage_ptr_dyn_storage.view_storage_controller_mut().unwrap();

        view_controller
            .set_input::<usize, i32>(input_storage_ptr)
            .unwrap();

        let view_keys: Vec<usize> = vec![0, 2, 4];

        view_controller
            .create_read_view::<usize, i32>(view_keys)
            .unwrap();
    }

    // Get access to the ViewStorage via a KeyItemStorage trait object and test its content using dynamic dispatch
    {
        let view_storage_ptr: StorageHandle<
            dyn KeyItemStorage<Key = usize, Item = i32>,
        > = view_storage_ptr_dyn_storage
            .clone()
            .cast_to_getitem_storage()
            .unwrap();

        // Get a guard to the dyn KeyItemStorage StorageHandle
        let guard = view_storage_ptr.try_read().unwrap();

        // Confirm that dyn dispatch based iteration works
        println!("Dyn dispatch view iteration: ");
        for (key, item) in guard.key_item_iter()
        {
            println!("{key}, {item}");
        }

        // Use get to confirm that the given view keys translate to the expected source values
        // View keys:     [0,  2,  4]
        // Source values: [0,1,2,3,4]

        assert_eq!(guard.get(0).unwrap(), &0);
        assert_eq!(guard.get(1).unwrap(), &2);
        assert_eq!(guard.get(2).unwrap(), &4);
    }

    // Test that the view can be iterated using static dispatch too
    {
        // Cast to a concrete ptr
        let storage_ptr: StorageHandle<KeyItemViewStorage<VecStorage<usize, i32>, usize, i32>> =
            view_storage_ptr_dyn_storage.cast_to_sized_storage().unwrap();

        let guard = storage_ptr.try_read().unwrap();

        println!("Dyn dispatch view iteration: ");

        let mut sum: i32 = 0;
        for (key, item) in guard.key_item_iter()
        {
            sum += item;
            println!("{key}, {item}");
        }

        assert_eq!(sum, 6);
    }
}

#[test]
fn view_storage_write_test()
{
    // Create the source storage
    let input_storage_ptr: StorageHandle<dyn Storage> = {
        let storage: VecStorage<usize, i32> = VecStorage::new_from_iter(vec![0, 1, 2, 3, 4]);
        let storage = Arc::new(RwLock::new(storage));

        let storage_ptr: StorageHandle<dyn Storage> = StorageHandle::new(
            storage.clone(),
            storage,
            TypeId::of::<usize>(),
            TypeId::of::<i32>(),
        );

        storage_ptr
    };

    // Create the view storage (Not the actual view yet)
    let mut view_storage_ptr_dyn_storage: StorageHandle<dyn Storage> = {
        let storage: KeyItemViewStorage<VecStorage<usize, i32>, usize, i32> = KeyItemViewStorage::new();
        let storage = Arc::new(RwLock::new(storage));

        let storage_ptr: StorageHandle<dyn Storage> = StorageHandle::new_with_view_controller(
            storage.clone(),
            storage,
            TypeId::of::<usize>(),
            TypeId::of::<i32>(),
        );

        storage_ptr
    };

    // Create a view using the supplied view keys
    {
        let view_controller: &mut ViewStorageController =
            view_storage_ptr_dyn_storage.view_storage_controller_mut().unwrap();

        view_controller
            .set_input::<usize, i32>(input_storage_ptr)
            .unwrap();

        let view_keys: Vec<usize> = vec![0, 2, 4];

        view_controller
            .create_write_view::<usize, i32>(view_keys)
            .unwrap();
    }

    // Get access to the ViewStorage via a KeyItemStorage trait object and test its content using dynamic dispatch
    {
        let view_storage_ptr: StorageHandle<
            dyn MutKeyItemStorage<Key = usize, Item = i32>,
        > = view_storage_ptr_dyn_storage
            .clone()
            .cast_to_mut_getitem_storage()
            .unwrap();

        // Get a guard to the dyn KeyItemStorage StorageHandle
        let mut guard = view_storage_ptr.try_write().unwrap();

        // Code pattern diverges from the view_storage_read_test below
        // Keep the above in in sync between the two tests with the exception 
        // of a few mut based or *_write* based methods calls and or trait casts
        // ---------------------------------------------------------------------

        let item_0 = guard.get_mut(0).unwrap();
        *item_0 = 10;

        // Confirm that dyn dispatch based iteration works
        println!("Dyn dispatch view iteration: ");
        for (key, item) in guard.key_item_iter()
        {
            println!("{key}, {item}");
        }

        // Use get to confirm that the given view keys translate to the expected source values
        // View keys:     [0,  2,    4]
        // Source values: [10, 1, 2, 3,4]

        assert_eq!(guard.get(0).unwrap(), &10); // This should now reflect our mutated first item which is 10 instead of 0
        assert_eq!(guard.get(1).unwrap(), &2);
        assert_eq!(guard.get(2).unwrap(), &4);
    }
}

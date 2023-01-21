use crate::{
    casting::cast_to_dyn_getkeyitemviewstorage,
    storage_traits::{ViewStorageSetup, KeyTrait, Storage, ItemTrait},
    Arw, SimpleResult, storage_handle::StorageHandle,
};

pub struct ViewStorageController
{
    // Design: Even though only view storages should go in here.
    // Having this as dyn Storage as opposed to a 
    // generic type reduces complexity of casting code. 
    view_storage: Arw<dyn Storage>,

    // Arw Justification
    // -----------------------------------------------------------
    // Arc: So that we can infallibly clone the ViewController 
    // Because the StorageHandle that owns it needs to be easily cloneable;
    // and so that the status stays in sync across clones of StorageHandle.
    // RwLock: So that we can use interior mutability without imposing 
    // a smart pointer around the whole StorageHandle that owns this type
    // Which would impose two layers of interior mutability on other fields 
    // of StorageHandle. Thats too much of an ergonomic hit.
    pub(super) status: Arw<InputStorageLockStatus>,
}

impl ViewStorageController
{
    pub fn new(
        base_storage: Arw<dyn Storage>,
        status: Arw<InputStorageLockStatus>,
    ) -> Self {
        Self {
            view_storage: base_storage,
            status,
        }
    }

    pub fn clear_view<Key, Item>(&mut self) -> SimpleResult<()>
    where
        Key: KeyTrait,
        Item: ItemTrait,
    {
        // Cast from Arw<dyn Storage> -> Arw<dyn KeyItemViewStorage>
        let storage: Arw<dyn ViewStorageSetup<Key = Key>> =
            cast_to_dyn_getkeyitemviewstorage::<dyn Storage, Key, Item>(self.view_storage.clone())?;

        let Ok(mut guard) = storage.try_write() 
        else {
            return Err("Failed to aquire view storage write guard".into());
        };

        guard.clear_view();
        
        let Ok(mut status_guard) = self.status.try_write() else {
            return Err("Failed to aquire write guard for ViewController's status".into());
        };

        *status_guard = InputStorageLockStatus::None;

        Ok(())
    }

    pub fn set_input<Key, Item>(&mut self, input_storage: StorageHandle<dyn Storage>) -> SimpleResult<()>
    where
        Key: KeyTrait,
        Item: ItemTrait,
    {
        let Ok(status_guard) = self.status.try_read() else {
            return Err("Failed to aquire read guard for ViewController's status".into());
        };

        if *status_guard != InputStorageLockStatus::None {
            return Err("Failed to set input. A read or write guard has already been aquired on the view. You must call clear before changing input".into());
        }

        // Cast from Arw<dyn Storage> -> Arw<dyn KeyItemViewStorage>
        let view_storage: Arw<dyn ViewStorageSetup<Key = Key>> =
            cast_to_dyn_getkeyitemviewstorage::<dyn Storage, Key, Item>(self.view_storage.clone())?;

        let Ok(mut view_storage_guard) = view_storage.try_write() 
        else {
            return Err("Failed to aquire view storage write guard".into());
        };

        let view_storage: Arw<dyn Storage> = input_storage.base_storage.clone();

        view_storage_guard.set_input_storage(view_storage);

        Ok(())
    }

    pub fn create_read_view<Key, Item>(&mut self, keys: impl IntoIterator<Item = Key> + 'static) -> SimpleResult<()>
    where
        Key: KeyTrait,
        Item: ItemTrait,
    {
        let Ok(mut status_guard) = self.status.try_write() else {
            return Err("Failed to aquire write guard for ViewController's status".into());
        };

        if *status_guard != InputStorageLockStatus::None {
            return Err("Failed to create view. A read or write guard has already been aquired on the view. You must call clear before changing input".into());
        }

        // Cast from Arw<dyn Storage> -> Arw<dyn KeyItemViewStorage>
        let view_storage_ptr: Arw<dyn ViewStorageSetup<Key = Key>> =
            cast_to_dyn_getkeyitemviewstorage::<dyn Storage, Key, Item>(self.view_storage.clone())?;

        let Ok(mut view_storage_guard) = view_storage_ptr.try_write() 
        else {
            return Err("Failed to aquire view storage write guard".into());
        };

        view_storage_guard.create_read_view(Box::new(keys.into_iter()))?;

        // Setting as Readable allows StorageHandle to take out try_read references to storage view
        *status_guard = InputStorageLockStatus::Readable;

        Ok(())
    }

    pub fn create_write_view<Key, Item>(&mut self, keys: impl IntoIterator<Item = Key> + 'static) -> SimpleResult<()>
    where
        Key: KeyTrait,
        Item: ItemTrait,
    {
        let Ok(mut status_guard) = self.status.try_write() else {
            return Err("Failed to aquire write guard for ViewController's status".into());
        };

        if *status_guard != InputStorageLockStatus::None {
            return Err("Failed to create view. A read or write guard has already been aquired on the view. You must call clear before changing input".into());
        }

        // Cast from Arw<dyn Storage> -> Arw<dyn KeyItemViewStorage>
        let view_storage_ptr: Arw<dyn ViewStorageSetup<Key = Key>> =
            cast_to_dyn_getkeyitemviewstorage::<dyn Storage, Key, Item>(self.view_storage.clone())?;

        let Ok(mut view_storage_guard) = view_storage_ptr.try_write() 
        else {
            return Err("Failed to aquire view storage write guard".into());
        };

        view_storage_guard.create_write_view(Box::new(keys.into_iter()))?;

        // Setting as Writable allows StorageHandle to take out try_write references to storage view
        *status_guard = InputStorageLockStatus::Writable;

        Ok(())
    }

    pub fn status(&self) -> SimpleResult<InputStorageLockStatus> {

        let Ok(status_guard) = self.status.try_read() else {
            return Err("Failed to aquire read guard for ViewController's status".into());
        };

        Ok(*status_guard)
    }
}

impl Clone for ViewStorageController
{
    fn clone(&self) -> Self {
        Self {
            view_storage: self.view_storage.clone(),
            status: self.status.clone(),
        }
    }
}

/// - NotSetup: StorageHandle should NEVER hand out references to a view storage if the controller is
///   in this state
/// - Readable: Has a read guard taken out on the input storage
/// - Writable: Has a write guard taken out on the input storage
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum InputStorageLockStatus {
    None, // View has not been created
    Readable,
    Writable,
}

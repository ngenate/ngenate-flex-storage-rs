// There are a number of things that rust can't cast out of the box when trait
// objects and or smart pointers are involved. These tests all cause compiler
// errors and are kept here for reference / educational purposes.
//
// Important: The module is disabled by default so to use it please uncomment it
// out in mod.rs of the casting module.

#![allow(unused_imports)]

#[cfg(test)]
mod tests {

    use std::sync::{Arc, RwLock};

    use ngenate_flex_storage::{
        storage_traits::{ItemSliceStorage, KeyItemStorage, KeyStorage, Storage},
        storage_types::{VecStorage},
        Arw,
    };

    use ngenate_flex_storage::casting::cast_to_dyn_getkeyitemstorage;

    ////////////////////////////////////////////////////////////////////////////
    // Rust Casting Limitation Examples
    ////////////////////////////////////////////////////////////////////////////


    // &&VecStorage<usize, i32> -> &dyn IntoIterator<IntoIter = _, Item = (usize, &i32)>
    // This cast can be done, though its not useful because the into_iter call cannot work
    // with dynamic dispatch / aka trait objects. So when working with iterators we will only
    // do so using static dispatch from now on. This example is left here commented out to show
    // the issue if trying to do something useful with IntoIterator as a dyn object
    // Important: We could of course make our own dynamic dispatch based IntoIterator trait
    // alternative that doesn't consume self but for now we want to focus on static dispatch for
    // iteration for performance reason
    // #[test]
    // fn into_iter_cast_compile_error() {
    //     let vec_storage: VecStorage<usize, i32> = VecStorage::new_from_iter(vec![1, 2, 3]);
    //
    //     // Can make the trait object - though we will see below we cant do anything useful with it
    //     let _data_access: &dyn IntoIterator<IntoIter = _, Item = &i32> = &&vec_storage;
    //
    //     // However, cannot convert this into an actual iterator because into_iter takes self
    //     // which is implicitly constrained as where Self::Sized and there for cannot support
    //     // dynamic dispatch calling. 
    //     for i in _data_access.into_iter() {}
    // }

    // This demonstrates that rust won't implicitly accept a more specified trait object than
    // the trait object expected in a function argument. Uncomment this to see the compiler error.
    // There is a trait upcast coercion initiative that may allow this in future
    // #[test]
    // fn upcast_fail() {
    //     let vec_storage: VecStorage<usize, i32> = VecStorage::new_from_iter(vec![1, 2, 3]);
    //
    //     // Simple cast from concrete ref to dyn ref
    //     {
    //         let storage: &dyn ItemSliceStorage<Item = i32> = &vec_storage;
    //         let storage: &dyn Storage = &storage;
    //     }
    // }

    // Another demonstration (This time adding in Arw smart pointers) that rust won't implicitly
    // accept a more specified trait object than the trait object expected in a function argument.
    // Uncomment this to see the compiler error
    // There may be a way to achieve this by using a generic type
    // constrained by either ?Sized and or Unsize<dyn Storage> but I couldn't get this to work yet.
    // #[test]
    // fn inter_trait_upcast_with_arw_compile_error() {
    //     let vec_storage: VecStorage<usize, i32> = VecStorage::new_from_iter(vec![1, 2, 3]);
    //
    //     // Prepare the source
    //     let storage: Arw<VecStorage<usize, i32>> = Arc::new(RwLock::new(vec_storage.clone()));
    //     let storage: Arw<dyn KeyStorage<Key = usize>> = storage;
    //
    //     let slice_storage: Arw<dyn KeyItemStorage<Key = usize, Item = i32>> =
    //         cast_to_dyn_getkeyitemstorage::<_, usize, i32>(storage).unwrap();
    //
    //     let guard = slice_storage.try_read().unwrap();
    //     assert_eq!(guard.get(0).unwrap(), &1);
    // }

    // #[test]
    // fn inter_trait_upcast() {
    //     let vec_storage: VecStorage<usize, i32> = VecStorage::new_from_iter(vec![1, 2, 3]);
    //
    //     // Prepare the source
    //     let storage: Arw<VecStorage<usize, i32>> = Arc::new(RwLock::new(vec_storage.clone()));
    //     let storage: Arw<dyn Storage> = storage;
    //
    //     // Cast
    //     let slice_storage: Arw<dyn KeyItemStorage<Key = usize, Item = i32>> =
    //         cast_to_dyn_getkeyitemstorage::<_, usize, i32>(storage).unwrap();
    //
    //     let storage: Arw<dyn Storage> = slice_storage;
    // }

    // This test shows that we get a compile error if we introduce a RwLock into an Arc as in
    // Arc<RwLock<dyn Storage>> and try to cast it to a concrete type. So a workaround is needed
    // which this crate has in the form of some unsafe casting code that is demonstrated
    // elsewhere This just shows the raw issue:
    // #[test]
    // fn downcast_fail_with_introduction_of_rwlock() {
    //     let vec_storage: VecStorage<usize, i32> = VecStorage::new_from_iter(vec![1, 2, 3]);
    //
    //     // FAILING BLOCK: Introducing a RwLock into an Arc causes a compile error.
    //     // Arc<RwLock<dyn Storage>> -> Arc<RwLock<VecStorage<usize, i32>>>
    //     {
    //         let storage: Arw<VecStorage<usize, i32>> = Arw::new(RwLock::new(vec_storage.clone()));
    //         let storage: Arw<dyn Storage> = storage;
    //         let storage = storage
    //             .downcast_arc::<VecStorage<usize, i32>>()
    //             .map_err(|_| "Shouldn't happen")
    //             .unwrap();
    //         assert_eq!(storage.num_items(), 3);
    //     }
    //
    //     // PASSING BLOCK: Arc<dyn Storage> -> Arc<VecStorage<usize, i32>>
    //     // This actually works no problem as long as we don't have the RwLock involved
    //     {
    //         // prepare the source
    //         let storage: Arc<VecStorage<usize, i32>> = Arc::new(vec_storage.clone());
    //         let storage: Arc<dyn Storage> = storage;
    //
    //         // cast
    //         let storage = storage
    //             .downcast_arc::<VecStorage<usize, i32>>()
    //             .map_err(|_| "Shouldn't happen")
    //             .unwrap();
    //
    //         assert_eq!(storage.len(), 3);
    //     }
    // }

    // Demonstrates that rust does not support trait upcast coercion out of the box
    // Though there is an experimental feature being worked on mentioned in the error
    // message below
    // #[test]
    // fn trait_upcast_coercion_error_test() {
    //     pub trait BaseTrait {
    //         fn foo(&self);
    //     }
    //
    //     pub trait ChildTrait: BaseTrait {}
    //
    //     pub struct Foo;
    //
    //     impl BaseTrait for Foo {
    //         fn foo(&self) {
    //             println!("called foo");
    //         }
    //     }
    //
    //     impl ChildTrait for Foo {}
    //
    //     fn take_storage(storage: &dyn BaseTrait) {
    //         storage.foo();
    //     }
    //
    //     let foo = Foo;
    //
    //     let base: &dyn BaseTrait = &foo;
    //     let child: &dyn ChildTrait = &foo;
    //
    //     take_storage(child);
    // }
}

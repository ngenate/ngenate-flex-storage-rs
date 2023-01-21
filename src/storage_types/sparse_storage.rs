use std::iter;
use std::{fmt::Debug, any::TypeId};

use std::mem::size_of;
use xsparseset::SparseSetVec;

use crate::storage_traits::{
    AsBytesBorrowed, ClearableStorage, ItemSliceStorage, ItemStorage, ItemTrait, KeyItemStorage,
    KeyStorage, MutItemSliceStorage, MutKeyItemStorage, Storage, KeyTypeIdNoSelf, ItemTypeIdNoSelf, KeyTrait
};

/// Sparse Storage that uses a vec to store the Sparse Keys
/// 
// #DESIGN
// The third party [SparseSetVec] is used internally for the actual sparse
// set implementation. This means that keys used for this storage must have
// Into<usize> and also implement Copy as that is also a constraint of
// the interior [SparseSetVec]
#[derive(Clone, Debug, Default)]
pub struct SparseSetVecStorage<Key, Item> 
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    data: SparseSetVec<Key, Item>,
}

////////////////////////////////////////////////////////////////////////////////
// Inherent methods
////////////////////////////////////////////////////////////////////////////////

impl<Key, Item> SparseSetVecStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    pub fn new() -> Self {
        assert!(Key::supports_index());

        Self {
            data: <_>::default(),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// Rust std traits impl
////////////////////////////////////////////////////////////////////////////////

impl<'a, Key, Item> IntoIterator for &'a SparseSetVecStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    type Item = (&'a Key, &'a Item);

    type IntoIter = std::iter::Zip<std::slice::Iter<'a, Key>, std::slice::Iter<'a, Item>>;

    fn into_iter(self) -> Self::IntoIter {
        let zipped = std::iter::zip(self.data.ids(), self.data.data());
        zipped
    }
}

////////////////////////////////////////////////////////////////////////////////
// Storage trait family impl
////////////////////////////////////////////////////////////////////////////////

impl<Key, Item> Storage for SparseSetVecStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn len(&self) -> usize {
        self.data.len()
    }
}

impl<Key, Item> KeyTypeIdNoSelf for SparseSetVecStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn key_type_id() -> std::any::TypeId {
        TypeId::of::<Key>()
    }
}

impl<Key, Item> ItemTypeIdNoSelf for SparseSetVecStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn item_type_id() -> std::any::TypeId {
        TypeId::of::<Item>()
    }
}

impl<Key, Item> KeyStorage for SparseSetVecStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    type Key = Key;

    fn contains(&self, key: Self::Key) -> bool {
        self.data.contains(key)
    }

    fn keys_iter(&self) -> Box<dyn Iterator<Item=Self::Key> + '_> {
        Box::new(self.data.ids().iter().cloned())
    }
}

impl<Key, Item> ItemStorage for SparseSetVecStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    type Item = Item;
}

impl<Key, Item> KeyItemStorage for SparseSetVecStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn get(&self, key: Key) -> Option<&Item> {
        self.data.get(key)
    }

    fn item_iter(&self) -> Box<dyn Iterator<Item = &Self::Item> + '_> {

        let iter = self.data.data().iter();

        Box::new(iter)
    }

    fn key_item_iter(&self) -> Box<dyn Iterator<Item = (Self::Key, &Self::Item)> + '_> {

        let ids_iter = self.data.ids().iter().cloned();
        let item_iter = self.data.data();

        let zip_iter = iter::zip(ids_iter, item_iter);

        Box::new(zip_iter)
    }
}

impl<Key, Item> ItemSliceStorage for SparseSetVecStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn as_item_slice(&self) -> &[Item] {
        self.data.data()
    }
}

impl<Key, Item> MutItemSliceStorage for SparseSetVecStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn as_mut_slice(&mut self) -> &mut [Item] {
        self.data.data_mut()
    }
}

impl<Key, Item> MutKeyItemStorage for SparseSetVecStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn insert(&mut self, key: Key, item: Item) {
        self.data.insert(key, item);
    }

    fn get_mut(&mut self, key: Self::Key) -> Option<&mut Self::Item> {
        self.data.get_mut(key)
    }
}

impl<Key, Item> ClearableStorage for SparseSetVecStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn clear(&mut self) {
        self.data.clear()
    }
}

impl<Key, Item> AsBytesBorrowed for SparseSetVecStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn byte_slice(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self.as_item_slice().as_ptr() as *const u8,
                self.as_item_slice().len() * size_of::<Item>(),
            )
        }
    }
}

#[cfg(test)]
mod tests {

    use super::SparseSetVecStorage;
    use crate::storage_traits::{KeyItemStorage, MutKeyItemStorage};

    #[test]
    fn test() {
        let mut storage_a: SparseSetVecStorage<usize, i32> = SparseSetVecStorage::new();

        let orig_entry_0 = 0;
        let orig_entry_1 = 1;

        storage_a.insert(0, orig_entry_0.clone());
        storage_a.insert(1, orig_entry_1.clone());

        let entry_0 = storage_a.get(0).unwrap();
        let entry_1 = storage_a.get(1).unwrap();

        assert_eq!(orig_entry_0, *entry_0);
        assert_eq!(orig_entry_1, *entry_1);

        println!("Implicit IntoIterator::into_iter loop:");
        for (id, item) in &storage_a {
            println!("{:?}", (id, item));
        }
    }
}

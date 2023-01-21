use std::any::TypeId;
use std::collections::hash_map::Iter;
use std::{collections::HashMap, fmt::Debug};

use crate::storage_traits::{
    ClearableStorage, ItemStorage, ItemTrait, ItemTypeIdNoSelf, KeyItemStorage, KeyStorage,
    KeyTrait, KeyTypeIdNoSelf, MutKeyItemStorage, Storage,
};

/// Sparse Storage that uses a vec to store the Sparse Keys
/// #DESIGN
/// The third party [xsparseset::SparseSetVec] is used internally for the actual sparse
/// set implementation. This means that keys used for this storage must have
/// [`Into<usize>`] and also implement Copy as that is also a constraint of
/// the interior [xsparseset::SparseSetVec]
#[derive(Clone, Debug, Default)]
pub struct HashMapStorage<Key, Item>
{
    data: HashMap<Key, Item>,
}

////////////////////////////////////////////////////////////////////////////////
// Inherent methods
////////////////////////////////////////////////////////////////////////////////

impl<Key, Item> HashMapStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    pub fn new() -> Self
    {
        // Unlike VecStorage, etc we don't need the keys to have index support
        // as HashMaps don't need to store their keys in indices.
        // So we don't need the assert below - leaving here for self documentation though
        // assert!(Key::supports_index());

        Self {
            data: <_>::default(),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// Rust std traits impl
////////////////////////////////////////////////////////////////////////////////

impl<'a, Key, Item> IntoIterator for &'a HashMapStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    type Item = (&'a Key, &'a Item);
    type IntoIter = Iter<'a, Key, Item>;

    #[inline]
    fn into_iter(self) -> Iter<'a, Key, Item>
    {
        self.data.iter()
    }
}

////////////////////////////////////////////////////////////////////////////////
// Storage trait family impl
////////////////////////////////////////////////////////////////////////////////

impl<Key, Item> Storage for HashMapStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn len(&self) -> usize
    {
        self.data.len()
    }
}

impl<Key, Item> KeyTypeIdNoSelf for HashMapStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn key_type_id() -> std::any::TypeId
    {
        TypeId::of::<Key>()
    }
}

impl<Key, Item> ItemTypeIdNoSelf for HashMapStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn item_type_id() -> std::any::TypeId
    {
        TypeId::of::<Item>()
    }
}

impl<Key, Item> KeyStorage for HashMapStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    type Key = Key;

    fn contains(&self, key: Self::Key) -> bool
    {
        self.data.contains_key(&key)
    }

    fn keys_iter(&self) -> Box<dyn Iterator<Item = Self::Key> + '_>
    {
        Box::new(self.data.keys().cloned())
    }
}

impl<Key, Item> ItemStorage for HashMapStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    type Item = Item;
}

impl<Key, Item> KeyItemStorage for HashMapStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn get(&self, key: Key) -> Option<&Item>
    {
        self.data.get(&key)
    }

    fn item_iter(&self) -> Box<dyn Iterator<Item = &Self::Item> + '_>
    {
        let iter = self.data.values();

        Box::new(iter)
    }

    fn key_item_iter(&self) -> Box<dyn Iterator<Item = (Self::Key, &Self::Item)> + '_>
    {
        let iter = self.data.iter().map(|(key, item)| (*key, item));

        Box::new(iter)
    }
}

impl<Key, Item> MutKeyItemStorage for HashMapStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn insert(&mut self, key: Key, item: Item)
    {
        self.data.insert(key, item);
    }

    fn get_mut(&mut self, key: Self::Key) -> Option<&mut Self::Item>
    {
        self.data.get_mut(&key)
    }
}

impl<Key, Item> ClearableStorage for HashMapStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn clear(&mut self)
    {
        self.data.clear()
    }
}

#[cfg(test)]
mod tests
{
    use super::HashMapStorage;
    use crate::storage_traits::{KeyItemStorage, MutKeyItemStorage};

    #[test]
    fn test()
    {
        let mut storage_a: HashMapStorage<usize, i32> = HashMapStorage::new();

        let orig_entry_0 = 0;
        let orig_entry_1 = 1;

        storage_a.insert(0, orig_entry_0.clone());
        storage_a.insert(1, orig_entry_1.clone());

        let entry_0 = storage_a.get(0).unwrap();
        let entry_1 = storage_a.get(1).unwrap();

        assert_eq!(orig_entry_0, *entry_0);
        assert_eq!(orig_entry_1, *entry_1);

        println!("Implicit IntoIterator::into_iter loop:");
        for (id, item) in &storage_a
        {
            println!("{:?}", (id, item));
        }
    }
}

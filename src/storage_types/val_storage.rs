use crate::storage_traits::{
    ItemSliceStorage, ItemStorage, MutItemSliceStorage, ItemTypeIdNoSelf, KeyTypeIdNoSelf, ItemTrait, KeyItemStorage, KeyStorage, Storage, AsFloatVec
};

use core::slice;
use std::any::TypeId;
use std::{fmt::Debug, marker::PhantomData};

use super::{key_to_index, index_to_key, KeyTrait};

#[derive(Debug, Clone, Default)]
pub struct ValStorage<Key, Item> {
    pub data: Item,
    key_phantom: PhantomData<Key>,
}

impl<Key, Item> ValStorage<Key, Item>
where
    Key: KeyTrait,
{
    pub fn new(val: Item) -> Self {
        assert!(Key::supports_index());

        Self {
            data: val,
            key_phantom: <_>::default(),
        }
    }
}

impl<Key, Item> Storage for ValStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn len(&self) -> usize {
        1
    }
}

impl<Key, Item> KeyTypeIdNoSelf for ValStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn key_type_id() -> std::any::TypeId {
        TypeId::of::<Key>()
    }
}

impl<Key, Item> ItemTypeIdNoSelf for ValStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn item_type_id() -> std::any::TypeId {
        TypeId::of::<Item>()
    }
}

impl<Key, Item> ItemStorage for ValStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    type Item = Item;
}

impl<Key, Item> ItemSliceStorage for ValStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    /// Returns the value as a single item slice
    fn as_item_slice(&self) -> &[Item] {
        slice::from_ref(&self.data)
    }
}

impl<Key, Item> MutItemSliceStorage for ValStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn as_mut_slice(&mut self) -> &mut [Item] {
        slice::from_mut(&mut self.data)
    }
}

////////////////////////////////////////////////////
// Key Storage Supertrait Impls
////////////////////////////////////////////////////

impl<Key, Item> KeyStorage for ValStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    type Key = Key;

    fn contains(&self, index: Self::Key) -> bool {
        0 == key_to_index(index)
    }

    fn keys_iter(&self) -> Box<dyn Iterator<Item=Self::Key> + '_> {

        // Returns an iterator that will return 0 for the sole key that this has and then exit
        let range_iter = (0..1).map(|v| index_to_key(v));
        Box::new(range_iter)
    }
}

impl<Key, Item> KeyItemStorage for ValStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn get(&self, key: Self::Key) -> Option<&Item> {
        let index: usize = key_to_index(key);

        if index == 0 {
            Some(&self.data)
        } else {
            None
        }
    }

    fn item_iter(&self) -> Box<dyn Iterator<Item = &Self::Item> + '_> {

        let iter = self .as_item_slice().iter();
        Box::new(iter)
    }

    fn key_item_iter(&self) -> Box<dyn Iterator<Item = (Self::Key, &Self::Item)> + '_> {

        let iter = self .as_item_slice().iter()
            .enumerate()
            .map(|(index, item)| (index_to_key(index), item));

        Box::new(iter)
    }
}

////////////////////////////////////////////////////

impl<Key, Item> AsFloatVec for ValStorage<Key, Item>
where
    Key: KeyTrait,
    Item: 'static + AsFloatVec + Sync + Send + Debug + Copy + Into<f32>,
{
    fn as_float_vec(&self) -> Vec<f32> {
        vec![self.data.into()]
    }
}

#[cfg(test)]
mod tests {

    use super::ValStorage;

    #[test]
    fn test() {
        let storage = ValStorage::<usize, i32>::new(1);

        let val = storage.data;

        assert_eq!(val, 1);
    }
}

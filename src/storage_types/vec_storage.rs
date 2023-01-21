//! VecStorage is a simple wrapper around [Vec] that implements traits from
//! [crate::storage_traits] where applicable.
//
// #DESIGN (Important)
// - [VecStorage] Does not try to introduce new semantics or substantially different abstractions
//   for handling a Vec instead, its main goal is to share some traits with other storage types
//   only where the fit is very natural and to provide some insulation away from the full std vec
//   API so that this API can grow only as needed and as big as needed. For example: Inserting
//   using a InsertKeyTrait that is also implemented on map like collections as well as this vec
//   would introduce a strange conflict in semantics. Additionally using special keys such as a
//   u128 is possible with map like collections and proper keys but pretending that key is an index
//   by bounding it with : Into<usize> would obviously remove most of the bits of that key if usize
//   <= 64bits.
// - The traits: KeyStorage, KeyItemStorage, MutKeyItemStorage are all not implemented because they
//   have semantics and their own requirements that are in some cases quite different to those of a
//   vec like storage such as this one. For example: Vecs traditionally insert either at or before
//   the given index (depending on the language std lib) and shift other elements to the right. In
//   contrast to this, HashMaps and their kind insert at the given key and don't affect any other
//   keys unless that key was added before.
//
// In summary, if true Key Item based semantics are required then a map like storages should be
// used. For example, this crate could have multiple map like storage types that all share Storage
// Map traits so there would still be uniformity of traits in those cases which is useful for
// interchangeability but just not here for a true vec like storage.

use crate::storage_traits::{
    AsBytesBorrowed, ClearableStorage, ItemSliceStorage, ItemStorage, ItemTrait,
    MutItemSliceStorage, Storage, ItemTypeIdNoSelf, KeyItemStorage, KeyTypeIdNoSelf, MutKeyItemStorage, KeyStorage
};

use std::{any::TypeId, marker::PhantomData, mem::size_of};

use super::{index_to_key, key_to_index, KeyTrait};

#[derive(Clone, Debug, Default)]
pub struct VecStorage<Key, Item> {
    data: Vec<Item>,

    // #DESIGN Unlike a normal Vec - Index phantom data is required so that
    // we can make trait objects of this type related to the key type that is used.
    index_phantom: PhantomData<Key>,
}

////////////////////////////////////////////////////////////////////////////////
// Inherent methods
////////////////////////////////////////////////////////////////////////////////

impl<Key, Item> VecStorage<Key, Item>
where
    Key: KeyTrait,
{
    pub fn new() -> Self {
        // Prevent the construction of this type if a non index supporting
        // Key has been passed in.
        assert!(Key::supports_index());

        Self {
            data: <_>::default(),
            index_phantom: <_>::default(),
        }
    }

    pub fn new_from_iter<I: IntoIterator<Item = Item>>(iter: I) -> Self {
        assert!(Key::supports_index());

        let mut data: Vec<Item> = Default::default();
        data.extend(iter);

        VecStorage {
            data,
            index_phantom: <_>::default(),
        }
    }

    // TODO: Consider changing this to Slice syntax and removing the set
    // because Vec doesn't have a set method
    pub fn set(&mut self, index: usize, item: Item) {
        self.data[index] = item;
    }

    pub fn push(&mut self, item: Item) {
        self.data.push(item);
    }

    // -------------------------------------------------

    /// A classic Vec like insert.
    ///
    /// Insert is also implemented for Vec as an Inherent impl as well as
    /// via [MutKeyItemStorage]. This is because this has Vector like semantics
    /// which inserts at an index location and shifts items to the right.
    /// but [MutKeyItemStorage::insert] has map like (Hash + Eq) key semantics
    /// which just inserts without shifting. Both types of semantics are useful
    /// and the name of this inherent method has been made more explicit to
    /// disambiguate with the trait based insert.
    pub fn insert_and_shift(&mut self, index: usize, item: Item) {
        self.data.insert(index, item);
    }

    // ---------------------------------------------------
}

////////////////////////////////////////////////////////////////////////////////
// Rust std traits impl
////////////////////////////////////////////////////////////////////////////////

impl<'a, Key, Item> IntoIterator for &'a VecStorage<Key, Item> {
    type Item = &'a Item;

    type IntoIter = std::slice::Iter<'a, Item>;

    fn into_iter(self) -> Self::IntoIter {
        let iter: std::slice::Iter<Item> = self.data.iter();
        iter
    }
}

////////////////////////////////////////////////////////////////////////////////
// Storage trait family impl
////////////////////////////////////////////////////////////////////////////////

impl<Key, Item> Storage for VecStorage<Key, Item>
where
    // Both of these need to be bound to these traits
    // for any VecStorage implement so that
    // we can guarantee to the compiler that they
    // include Send + Sync
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn len(&self) -> usize {
        self.data.len()
    }
}

impl<Key, Item> KeyTypeIdNoSelf for VecStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn key_type_id() -> std::any::TypeId {
        TypeId::of::<Key>()
    }
}

impl<Key, Item> ItemTypeIdNoSelf for VecStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn item_type_id() -> std::any::TypeId {
        TypeId::of::<Item>()
    }
}

impl<Key, Item> ItemStorage for VecStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    type Item = Item;
}

impl<Key, Item> KeyStorage for VecStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    type Key = Key;

    fn contains(&self, key: Self::Key) -> bool {
        let index: usize = key_to_index(key);
        index < self.data.len()
    }

    fn keys_iter(&self) -> Box<dyn Iterator<Item = Self::Key> + '_> {
        // Return the indices as keys by using a simple range iterator
        // Design: Keys need to be returned by value because a VecStorage
        // has no stored keys to return by reference from. Only Indices which
        // can be converted to Keys transiently during iteration.
        let range_iter = (0..self.data.len()).map(|v| index_to_key(v));
        Box::new(range_iter)
    }
}

impl<Key, Item> KeyItemStorage for VecStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn get(&self, index: Self::Key) -> Option<&Self::Item> {
        self.data.get(key_to_index(index))
    }

    fn key_item_iter(&self) -> Box<dyn Iterator<Item = (Self::Key, &Self::Item)> + '_> {
        let iter = self
            .data
            .iter()
            .enumerate()
            .map(|(index, item)| (index_to_key(index), item));

        Box::new(iter)
    }

    fn item_iter(&self) -> Box<dyn Iterator<Item = &Self::Item> + '_> {

        let iter = self
            .data
            .iter();

        Box::new(iter)
    }
}

impl<Key, Item> MutKeyItemStorage for VecStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    /// Inserts item into the storage at the Key as Index location
    /// and resizes the vector before insertion if Key as Index > VecStorage.len()
    /// This means that default items will automatically be created via [Default]
    /// for new slots and existing items will be cloned into the newly sized storage.
    /// #Design
    /// This method uses Clone + Default and is the primary reason for these two
    /// being added into [KeyTrait]
    fn insert(&mut self, key: Key, item: Item) {
        let index: usize = key_to_index(key);

        if index > self.data.len() {
            self.data.resize(index, Item::default());
        }

        self.data.insert(index, item);
    }

    fn get_mut(&mut self, key: Self::Key) -> Option<&mut Self::Item> {

        let index: usize = key_to_index(key);
        self.data.get_mut(index)
    }
}

impl<Key, Item> ItemSliceStorage for VecStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn as_item_slice(&self) -> &[Item] {
        self.data.as_slice()
    }
}

impl<Key, Item> MutItemSliceStorage for VecStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn as_mut_slice(&mut self) -> &mut [Item] {
        self.data.as_mut_slice()
    }
}

impl<Key, Item> ClearableStorage for VecStorage<Key, Item>
where
    Key: KeyTrait,
    Item: ItemTrait,
{
    fn clear(&mut self) {
        self.data.clear()
    }
}

////////////////////////////////////////////////////////
// Other Trait Impls
////////////////////////////////////////////////////////

impl<Key, Item> AsBytesBorrowed for VecStorage<Key, Item>
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

    use crate::storage_traits::KeyItemStorage;

    use super::VecStorage;

    #[test]
    fn test() {
        let mut storage_a: VecStorage<usize, i32> = VecStorage::new();

        let orig_entry_0 = 0;
        let orig_entry_1 = 1;

        // Test the inherent impl methods (Similar to std::vec insert and get)
        {
            storage_a.insert_and_shift(0, orig_entry_0.clone());
            storage_a.insert_and_shift(1, orig_entry_1.clone());

            let entry_0 = storage_a.get(0).unwrap();
            let entry_1 = storage_a.get(1).unwrap();

            assert_eq!(orig_entry_0, *entry_0);
            assert_eq!(orig_entry_1, *entry_1);
        }

        // Test KeyValAccess based super trait methods (Similar to std::vec insert and get)
        {
            storage_a.insert_and_shift(0, orig_entry_0.clone());
            storage_a.insert_and_shift(1, orig_entry_1.clone());

            let entry_0 = storage_a.get(0).unwrap();
            let entry_1 = storage_a.get(1).unwrap();

            assert_eq!(orig_entry_0, *entry_0);
            assert_eq!(orig_entry_1, *entry_1);
        }

        let iter = storage_a.into_iter();

        println!("Implicit IntoIterator::into_iter loop:");
        for item in iter.enumerate() {
            println!("{:?}", item);
        }
    }
}

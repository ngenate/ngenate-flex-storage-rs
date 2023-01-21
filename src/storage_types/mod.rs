//! A range of storage types intended to work well with threading and with a focus on shared traits
//! and tooling to promote richer trait based programming via either static or dynamic dispatch.
//! For more information see crate level documentation [crate]

mod hashmap_storage;
mod sparse_storage;
mod val_storage;
mod vec_storage;
mod view;

pub use hashmap_storage::*;
pub use sparse_storage::*;
pub use val_storage::*;
pub use vec_storage::*;
pub use view::*;

use crate::storage_traits::KeyTrait;

pub fn key_to_index<Key: KeyTrait>(key: Key) -> usize {
    if let Ok(val) = key.try_into() {
        val
    } else {
        panic!("Key could not be converted to usize");
    }
}

pub fn index_to_key<Key: KeyTrait>(index: usize) -> Key {
    if let Ok(val) = index.try_into() {
        val
    } else {
        panic!("Key could not be converted to usize");
    }
}

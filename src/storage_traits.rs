//! Provides shared functionality and classification to storage types.
//!
//! # Design
//! - Must be [Send] and [Sync] so that they are compatible with threading and the DowncastSync base
//!   trait from downcast_rs
//! - Must be 'static due to [Storage]: [DowncastSync] which is ultimately bound to: 'static
//! - Associated types or generics if used must not be bound in the trait definitions themselves as
//!   we wish to leave these up to implementors. Eg: VecStorage needs its key to have +
//!   [`Into<usize>`] as part of its bound and other storage types that are not indexable such as a
//!   [std::collections::HashMap] require there keys to implement [Hash], [Eq], etc
//! - A single KeyTrait needed to be used instead of having dual key traits: [KeyTrait] and
//!   IndexTrait. The reason for using a single trait that has Index like ([`TryInto<usize>`]) AND Key
//!   ([Hash] + [Eq]) like bounds is that having the two separated traits caused considerable
//!   ergonomic complexity around casting from a base trait such as storage to a supertrait
//!   (child-trait) such as [KeyItemStorage] as essentially two different casting functions were
//!   required for each cast to a child-trait. A macro could be used to help make it feel like just
//!   one function but the trait for the key still needed to be specified and the two different
//!   underlying cast functions PER cast to trait obj requirement would still be there doubling all
//!   the casting functions. Its possible when rust has improved dyn object casting that I wont have
//!   this issue and I may be able to bring back the two trait approach if I think that the Semantic
//!   win is justifies it.

use crate::{Arw, SimpleResult};
use downcast_rs::{impl_downcast, DowncastSync};
use std::any::TypeId;

/// Implements [KeyTrait] for the list of given types
//
// # Design
// A macro is used here to take place of Blanket impl. Blanket impl isn't feasible to use here as
// we need to implement it differently for two different sets of types. The two different sets are
// types that support being used as indices such as u18,16,usize, and types that don't such as u64
// and u128. Blanket implement has issues when trying to discriminate between these two sets. Some
// form of impl specialization may work in future but for now I couldn't get min_specialization to
// like one of the trait bounds (I think it was TryInto or possibly Into)
macro_rules! impl_key_trait {

    ( [$($t:ty),*], $supports_index:ident ) => {

        $( impl KeyTrait for $t
        {
            fn supports_index() -> bool {
                $supports_index
            }

        }) *
    }
}

// Note about not using the unstable trait_alias feature
// I was going to use this feature for Item and Key bounds but have decided for now to use
// KeyTrait and ItemTrait super traits instead until or if a strong use case emerges to the
// contrary.

/// Represents storage Keys that are able to be used on either Mappable or Indexable Storages
/// Only keys that can be converted into usize without losing any precision are indexable.
/// These keys will be no-op conversions to usize. Keys such as u128 can't serve this purpose but
/// can still be used for keys in Mappable storages.
/// # Trait Bounds
/// * [Sync] + [Send] to be maximally compatible with threading
/// * [Copy] because certain internal storage collections such as [Vec] and [xsparseset]
///   require keys to be [Copy]
/// * [Ord] so that we can sort keys when needed
/// * [`TryInto<usize>`] and [`TryFrom<usize>`] are required to convert to and from indices for storages
///   that use indices as keys These are Try because not all Keys can be converted into a sensible
///   index. eg. [u128] cant be used to lookup an index in a [crate::storage_types::VecStorage]
///   because its indices are capped out at [usize].
pub trait KeyTrait:
    Clone
    + Copy
    + Sync
    + Send
    + TryInto<usize>
    + TryFrom<usize>
    + std::hash::Hash
    + Eq
    + Ord
    + std::fmt::Debug
    + 'static
{
    fn supports_index() -> bool;
}

// Impl KeyTrait for types that can be converted to usize without precision loss
impl_key_trait!([u8, u16, usize], true);

// Impl KeyTrait for types that cannot be converted to our index type (usize) without precision loss
impl_key_trait!([i8, i16, i32, i64, i128, u32, u64, u128], false);

/// # Trait Bounds
/// * [Sync] + [Send] to be maximally compatible with threading
/// * [Default] so that storages like [crate::storage_types::VecStorage] can have default entries
///   populated if keys are inserted at sparse locations. The decision to introduce default was a
///   little difficult because I didn't want to impose too many requirements on what Items end users
///   could ultimately use but in exchange, VecStorage can now participate as a first class citizen
///   with the other storage types in terms of item insertion. And additionally, this crate
///   prioritizes maximal sharing of traits between storage types to allow for maximum storage type
///   interchangeability
pub trait ItemTrait: Sync + Send + Default + Clone + 'static {}
impl<T> ItemTrait for T where T: Sync + Send + Default + Clone + 'static {}

pub trait Storage: DowncastSync
{
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool
    {
        self.len() == 0
    }
}

impl_downcast!(sync Storage);

pub trait ClearableStorage: Storage
{
    fn clear(&mut self);
}

/// Storage that can be accessed by Key.
//
// #DESIGN
// - This trait is useful for keeping a collection of storage trait objects with only the type of
//   Key is known.
pub trait KeyStorage: Storage
{
    type Key;

    fn contains(&self, key: Self::Key) -> bool;

    // # Design
    // All storage types need to return cloned keys because
    // of [crate::storage_types::VecStorage] which has no actual stored keys and there
    // for can only return by value for its indices as keys.
    fn keys_iter(&self) -> Box<dyn Iterator<Item = Self::Key> + '_>;
}

pub trait ItemStorage: Storage
{
    type Item;

    fn item_type_id(&self) -> TypeId
    {
        TypeId::of::<Self::Item>()
    }
}

pub trait KeyItemStorage: KeyStorage + ItemStorage
{
    fn get(&self, key: Self::Key) -> Option<&Self::Item>;

    fn item_iter(&self) -> Box<dyn Iterator<Item = &Self::Item> + '_>;

    /// Return an iterator over (key, &Item) tuples.
    //
    // #Internal Design
    // Keys must be returned by value because there are some storages such as
    // [crate::storage_types::VecStorage] that don't have separately stored keys. In the case
    // of [crate::storage_types::VecStorage] its indices are converted to keys and there for
    // cannot be returned by reference. This pushes the requirement onto all storages that
    // implement this method to maintain a common interface.
    fn key_item_iter(&self) -> Box<dyn Iterator<Item = (Self::Key, &Self::Item)> + '_>;
}

pub trait MutKeyItemStorage: KeyItemStorage + ClearableStorage
{
    fn get_mut(&mut self, key: Self::Key) -> Option<&mut Self::Item>;

    // TODO: This needs to return a SimpleResult in the case of an unmatched key
    fn insert(&mut self, key: Self::Key, item: Self::Item);

    // TODO: Need to implement a mutable iterator here
    // fn key_item_iter_mut(&mut self) -> Box<dyn Iterator<Item = (Self::Key, &mut Self::Item)> +
    // '_>;
}

/// Provides common read only functionality for a map
pub trait ItemSliceStorage: ItemStorage
{
    fn as_item_slice(&self) -> &[Self::Item];
}

pub trait MutItemSliceStorage: ItemSliceStorage
{
    fn as_mut_slice(&mut self) -> &mut [Self::Item];
}

/// This trait is deliberately narrow in scope as this is only intended to be used by StorageHandle
/// and unit tests within ViewStorage
pub trait ViewStorageSetup: KeyStorage + ClearableStorage
{
    fn clear_view(&mut self);

    fn set_input_storage(&mut self, input: Arw<dyn Storage>);

    fn get_input_storage(&self) -> Option<Arw<dyn Storage>>;

    fn create_read_view(&mut self, keys: Box<dyn Iterator<Item = Self::Key>>) -> SimpleResult<()>;

    fn create_write_view(&mut self, keys: Box<dyn Iterator<Item = Self::Key>>) -> SimpleResult<()>;
}

// There are two traits offer alternate techniques for converting to more primitive data The
// [AsBytesBorrowed] which returns a slice (borrowed data) is used for Vec<T> which is used for
// buffer data at the moment because the WebGl API for supplying data takes borrowed data which is
// reasonable because buffers could get pretty big. Types such as Vector3 use [`AsVec`] trait
// because they copy their data in to the WebGL API in calls such as uniformf4v

pub trait AsBytesBorrowed
{
    fn byte_slice(&self) -> &[u8];
}

pub trait AsFloatVec
{
    fn as_float_vec(&self) -> Vec<f32>;
}

// These traits allow us to extract type ID information for cases where we only have access to a
// single generic storage type parameter and no instances or separate Key and Item Items

pub trait KeyTypeIdNoSelf
{
    fn key_type_id() -> TypeId;
}

pub trait ItemTypeIdNoSelf
{
    fn item_type_id() -> TypeId;
}

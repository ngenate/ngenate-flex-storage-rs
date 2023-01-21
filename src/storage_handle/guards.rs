//! Storage guards to provide RAII read and write access to storage types.
//!
//! # Internal Design 
//! - They are currently light weight wrapper guards around std [RwLockReadGuard] and [RwLockWriteGuard] 
//! - They serve as future proofing architecture in case custom code needs to be run when a guard is taken 
//!   out or dropped or for runtime tracking or debugging purposes.

use std::{sync::{RwLockReadGuard, RwLockWriteGuard}, ops::{Deref, DerefMut}};
use crate::storage_traits::Storage;

////////////////////////////////////////////////
// Storage Read Guard
////////////////////////////////////////////////

/// A Wrapper guard around [std::sync::RwLockReadGuard] so that 
/// a custom drop function can be called to trigger the release of 
/// indirectly borrowed / locked resources. 
///
/// # Design [crate::storage_types] is one main justification for needing this as view storage 
/// locks resources it doesn't own while it uses them and this helps release the locks
pub struct StorageReadGuard<'a, S> 
where
S: Storage + ?Sized + 'a,
{
    inner_guard: RwLockReadGuard<'a, S>
}

impl<'a, S> StorageReadGuard<'a, S> 
where
S: Storage + ?Sized + 'a,
{
    pub fn new(inner_guard: RwLockReadGuard<'a, S>) -> Self { Self { inner_guard } }
}

impl<'a, S> Deref for StorageReadGuard<'a, S> 
where
S: Storage + ?Sized,
{
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.inner_guard
    }
}

////////////////////////////////////////////////
// Storage Write Guard
////////////////////////////////////////////////

/// A Storage Read Guard that dereferences into the inner Storage
pub struct StorageWriteGuard<'a, S> 
where
S: Storage + ?Sized + 'a,
{
    inner_guard: RwLockWriteGuard<'a, S>
}

impl<'a, S> StorageWriteGuard<'a, S> 
where
S: Storage + ?Sized + 'a,
{
    pub fn new(inner_guard: RwLockWriteGuard<'a, S>) -> Self { Self { inner_guard } }
}

impl<'a, S> Deref for StorageWriteGuard<'a, S> 
where
S: Storage + ?Sized,
{
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.inner_guard
    }
}

/// A Storage Write Guard that dereferences into the inner Storage
impl<'a, S> DerefMut for StorageWriteGuard<'a, S> 
where
S: Storage + ?Sized,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner_guard
    }
}


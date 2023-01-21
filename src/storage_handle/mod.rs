//! A Smart Pointer to any Storage type that implements [crate::storage_traits::Storage]
//! See [StorageHandle] for details

pub mod handle;
mod guards;
mod view_storage_controller;

pub use handle::*;
pub use guards::*;
pub use view_storage_controller::*;

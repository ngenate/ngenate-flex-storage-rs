//! Flex Storage empowers API's to be more abstract over data by focusing on shared traits and
//! flexible casting. It provides Storage Handles to point to either concrete Storage types or trait
//! objects depending on if dynamic or static dispatch is needed, and casting infrastructure to
//! perform inter trait-object casts, trait-object to type casts and type un-sizing to
//! trait-objects.
//!
//! The crate was made to support a primary use case of dataflow processing within a node based
//! visual programming environment where the intention is to be able to use storage types as inputs
//! for processing which can be switched at runtime with interchangeable storage handles. Such a
//! high degree of runtime flexibility comes with some added API complexity as well as performance
//! considerations so consider a simpler static dispatch oriented workflow if most of your
//! storage design can be determined at compile time.
//!
//! # Features
//!
//! * Support for both dynamic and static dispatch though though dynamic dispatch workflows have had
//!   more work.
//! * Flexible casting between any type or trait object within the Storage trait family.
//! * Can be used to hold multiple handles to the same storage where each pointer can represent the
//!   storage as a different trait object or concrete type to fit the use case. This is ideal for
//!   graph based data processing.
//! * Primary use case is multithreaded so all storage types and handles are Send + Sync and use
//!   Arc<RwLock<StorageType>> internally within StorageHandles

// ----------------------------------------------------------------------------------------------
//
// # Internal Design
//
// This crate brings quite a bit of custom infrastructure with it to make casting storages as
// capable and simple as possible. The casting capability offered by this crate is what I hope that
// the rust language will evolve to be able to offer in the future, though it seems that there is
// no clear timelines on that. The following are some resources on work underway in various areas
// related to DSTs, fat pointers, and casting in general:
//
// See orig issue here: https://github.com/rust-lang/rust/issues/27732
// Latest issue here: https://github.com/rust-lang/rust/issues/18598
// Decent summary also here: https://rust-lang.github.io/rfcs/0982-dst-coercion.html
// dyn async traits Zulip here: https://rust-lang.zulipchat.com/#narrow/stream/187312-wg-async/topic/dyn.20async.20traits
//
// ## Safety
//
// The crate has a single unsafe function for casting that involves a RwLock.
// See [casting::dyn_storage_into_sized] for further documentation and implementation.
//
// ## Unstable Features
//
// ### ptr_metadata
//
// Casting using downcast_rs or Any involving Arc<RwLock<dyn Storage>> is not possible due to the
// RwLock which does not have a Coerce unsized implementation and is not mentioned in the
// documentation of Any or downcast_rs as being supported for downcast-ability when within an Arc.
// So there for I created a custom unsafe casting function [casting::dyn_storage_into_sized] that
// needs to get at the parts of a fat pointer using the ptr_metadata feature so that we can perform
// our own cast involving types like Arc<RwLock<dyn Storage>>.
//
// ## Alternatives to using unsafe code and ptr_metadata
//
// See Internal Design documentation in [StorageHandle] for discussion on this.

#![allow(dead_code)]
#![feature(ptr_metadata)]

// -------------------------------------------------------

pub mod casting;
pub mod storage_handle;
pub mod storage_traits;
pub mod storage_types;

use std::sync::{Arc, RwLock};

// ------------------------
// Type Aliases
// ------------------------

pub type Rw<T> = RwLock<T>;

/// Arc Read Write lock pointer
pub type Arw<T> = Arc<RwLock<T>>;

/// Optional Arc Read Write lock pointer
pub type OArw<T> = Option<Arc<RwLock<T>>>;

// -------------------------

// TODO: #LOW Consider replacing this with anyhow, etc
pub type SimpleResult<T> = Result<T, String>;

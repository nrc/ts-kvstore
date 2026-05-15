//! # An in-memory KV store for the Rust Tailscale client.
//!
//! ## Concepts
//!
//! - Typed KVs and schema macros
//! - Singletons and tables
//! - Raw and transactional APIs
//! - Ownership
//! 
//! ## Implementation
//! 
//! - A lot of stuff is pub just for macros; shouldn't be used
//! - External hash and hasher for singletons

use std::sync::RwLock;

mod hasher;
mod iter;
mod raw;
#[doc(hidden)]
pub mod schema;
#[doc(hidden)]
pub mod storage;
mod transactions;

pub struct KvStore<TableStorage: schema::GeneratedStorage> {
    storage: RwLock<storage::Storage<TableStorage>>,
}

impl<TableStorage: schema::GeneratedStorage> KvStore<TableStorage> {
    #[doc(hidden)]
    pub fn new_with_storage(storage: RwLock<storage::Storage<TableStorage>>) -> Self {
        KvStore { storage }
    }
}

pub type Owner = &'static str;

#[track_caller]
fn assert_owner(_owner: Owner) {
    todo!()
}

#[derive(Debug, Clone)]
pub enum Error {
    AlreadyInit(Owner),
    NotPresent,
}

pub type Result<T> = std::result::Result<T, Error>;

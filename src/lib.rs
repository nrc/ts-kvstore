//! # An in-memory KV store for the Rust Tailscale client.
//!
//! ## Concepts
//!
//! - Typed KVs and schema macros
//! - Singletons and tables
//! - Raw and transactional APIs
//! - Ownership

use std::sync::RwLock;

mod hasher;
mod iter;
mod raw;
mod schema;
mod storage;
mod transactions;

#[allow(private_bounds)]
pub struct KvStore<TableStorage: schema::GeneratedStorage> {
    storage: RwLock<storage::Storage<TableStorage>>,
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

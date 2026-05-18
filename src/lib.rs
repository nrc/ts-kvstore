//! # ts-kvstore
//!
//! An in-memory, async, concurrent, and strongly-typed KV store for the Rust Tailscale client.
//!
//! Example:
//!
//! ```rust
//! # use ts_kvstore::{Owner, singleton, tables};
//! # const OWNER: Owner = "owner";
//!
//! singleton!(foo("a", &'static str, u64));
//! tables!(Nodes(u32, String));
//!
//! pub fn main() {
//!     let store = KvStore::new();
//!
//!     store.insert::<foo>(OWNER, 42);
//!
//!     let nodes = store.table::<Nodes>(OWNER);
//!     nodes.insert(4, "a".to_owned());
//!     nodes.insert(0, "b".to_owned());
//!     nodes.insert(10, "c".to_owned());
//!     nodes.insert(400, "d".to_owned());
//!
//!     assert_eq!(nodes.len(), 4);
//!
//!     println!("singleton: {}, row 4: {}", store.get::<foo>(OWNER).unwrap(), nodes.get(&4).unwrap());
//! }
//! ```
//!
//! ## Concepts
//!
//! - Typed KVs and schema macros
//! - Singletons and tables
//! - Raw and transactional APIs, transactional guarantees, RO transactions
//! - Ownership
//! - async
//!
//! ## Implementation
//!
//! - A lot of stuff is pub just for macros; shouldn't be used
//! - External hash and hasher for singletons, SinValue
//! - KvStore wrapper around a KvStore

use std::sync::RwLock;

mod hasher;
mod iter;
mod raw;
#[doc(hidden)]
pub mod schema;
#[doc(hidden)]
pub mod storage;
mod transactions;

/// A key-value store. See the crate docs for details. Its schema is described by `TableStorage`.
pub struct KvStore<TableStorage: schema::GeneratedStorage> {
    /// All data is stored behind the RW lock (see `storage` and `schema` modules).
    storage: RwLock<storage::Storage<TableStorage>>,
}

impl<TableStorage: schema::GeneratedStorage> KvStore<TableStorage> {
    #[doc(hidden)]
    /// Constructor intended to be used by macros. Avoid using this and prefer to use the generated
    /// `new` for a specialized `KvStore`.
    pub fn new_with_storage(storage: RwLock<storage::Storage<TableStorage>>) -> Self {
        KvStore { storage }
    }
}

/// A token indicating ownership of a KV singleton or table. See crate docs for what ownership means
/// for a store.
pub type Owner = &'static str;

#[track_caller]
fn assert_owner(_owner: Owner) {
    todo!()
}

/// An error from a [`KvStore`].
#[derive(Debug, Clone)]
pub enum Error {
    /// A table was expected to not be initialized, but was by the specifed `Owner`.
    AlreadyInit(Owner),
    /// A key was expected to be present in the store, but was not.
    NotPresent,
}

/// `Result` alias for a KvStore [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

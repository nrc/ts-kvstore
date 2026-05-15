use crate::{Owner, schema};
use std::{
    any::Any,
    collections::HashMap,
    hash::{BuildHasher, RandomState},
    sync::Arc,
};

/// Where we store the data.
#[doc(hidden)]
pub struct Storage<TableStorage: schema::GeneratedStorage> {
    /// Singleton key/value pairs. Hashing of keys is done 'externally', so the key in the table
    /// is the result of hashing. We use `NoopU64Builder` to avoid re-hashing. See [`SinValue`] for
    /// how values are represented in the store.
    pub(crate) singletons: HashMap<u64, (Owner, SinValue), crate::hasher::NoopU64Builder>,
    /// Used to build a hasher to hash singleton keys.
    singleton_hash_builder: RandomState,
    /// Storage for tabular data. The concrete type will be macro-generated, see the [`crate::schema`]
    /// module.
    pub(crate) tables: TableStorage,
}

impl<TableStorage: schema::GeneratedStorage> Storage<TableStorage> {
    /// Create a new storage with no data.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Storage {
            singletons: HashMap::with_hasher(crate::hasher::NoopU64Builder),
            singleton_hash_builder: RandomState::new(),
            tables: TableStorage::default(),
        }
    }

    /// Hash a singleton key.
    pub(crate) fn hash(&self, thing: impl std::hash::Hash) -> u64 {
        self.singleton_hash_builder.hash_one(thing)
    }

    /// Retrive a singleton value and its owner from the store using the given pre-hashed key.
    pub(crate) fn get_singleton_value(
        &self,
        key: impl std::hash::Hash,
    ) -> Option<&(Owner, SinValue)> {
        self.singletons.get(&self.hash(key))
    }

    /// Retrive a singleton value and its owner from the store using the given pre-hashed key.
    pub(crate) fn get_singleton_value_mut(
        &mut self,
        key: impl std::hash::Hash,
    ) -> Option<(&Owner, &mut SinValue)> {
        self.singletons
            .get_mut(&self.hash(key))
            .map(|&mut (ref o, ref mut v)| (o, v))
    }
}

/// Internal storage for singleton values.
#[doc(hidden)]
#[derive(Default)]
pub enum SinValue {
    /// Tombstone value. Used where we want to specify an owner but don't have value yet.
    #[default]
    None,
    // TODO add other special cases
    /// A single, inline `u64`.
    U64(u64),
    /// A boxed value (i.e., a pointer is stored in the store).
    Box(Box<dyn Any + Send + Sync>),
    /// A shared reference in the store.
    Arc(Arc<dyn Any + Send + Sync>),
    /// A static reference in the store.
    Ref(&'static (dyn Any + Send + Sync)),
}

/// Tabular data in the KV store, there will be one of these for each logical table in the storage
/// implementing `TableStorage` in [`Storage`].
#[doc(hidden)]
#[derive(Default)]
pub struct Table<D: schema::DataDesc> {
    /// Owner of the table.
    pub(crate) owner: Option<Owner>,
    /// KV data.
    pub(crate) data: HashMap<D::Key, D::Value>,
}

use crate::{Owner, schema};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::Arc,
};

/// Where we store the data.
#[doc(hidden)]
pub struct Storage<TableStorage: schema::GeneratedStorage> {
    /// The key is the TypeId of the generated marker type for the KV data. See [`SinValue`] for
    /// how values are represented in the store.
    pub(crate) singletons: HashMap<TypeId, (Owner, SinValue)>,
    /// Storage for tabular data. The concrete type will be macro-generated, see the [`crate::schema`]
    /// module.
    pub(crate) tables: TableStorage,
}

impl<TableStorage: schema::GeneratedStorage> Storage<TableStorage> {
    /// Create a new storage with no data.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Storage {
            singletons: HashMap::new(),
            tables: TableStorage::default(),
        }
    }

    /// Retrieve a singleton value from the store using the given type-key.
    pub(crate) fn get_singleton_value(&self, key: &TypeId) -> Option<&SinValue> {
        self.singletons.get(key).map(|(_, v)| v)
    }

    /// Retrieve a singleton value from the store using the given type-key.
    pub(crate) fn get_singleton_value_mut(&mut self, key: &TypeId) -> Option<&mut SinValue> {
        self.singletons.get_mut(key).map(|&mut (_, ref mut v)| v)
    }

    /// Retrieve the owner of a singleton KV pair using the given type-key.
    #[cfg(debug_assertions)]
    pub(crate) fn get_singleton_owner(&self, key: &TypeId) -> Option<Owner> {
        self.singletons.get(key).map(|(o, _)| *o)
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
pub struct Table<D: schema::TableDesc> {
    /// Owner of the table.
    pub(crate) owner: Option<Owner>,
    /// KV data.
    pub(crate) data: HashMap<D::Key, D::Value>,
}

impl<D: schema::TableDesc> Default for Table<D> {
    fn default() -> Self {
        Self {
            owner: None,
            data: HashMap::new(),
        }
    }
}

impl<D: schema::TableDesc> Table<D> {
    pub fn assert_or_set_owner(&mut self, owner: Owner) {
        match &self.owner {
            Some(prev_owner) => debug_assert_eq!(
                *prev_owner, owner,
                "Ownership violation: expected {prev_owner}, found {owner}"
            ),
            None => {
                self.owner = Some(owner);
            }
        }
    }

    pub fn assert_owner(&mut self, owner: Owner) {
        if let Some(prev_owner) = &self.owner {
            debug_assert_eq!(
                *prev_owner, owner,
                "Ownership violation: expected {prev_owner}, found {owner}"
            );
        }
    }
}

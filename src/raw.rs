//! KvStore non-transactional API.

use crate::{
    Error, KvStore, Owner, Result, iter::TableIterator, schema, singleton::OptSingletonValue,
    storage::SinValue,
};
use std::{
    borrow::Borrow,
    hash::Hash,
    marker::PhantomData,
    sync::{Arc, RwLockReadGuard},
};

impl<TableStorage: schema::GeneratedStorage> KvStore<TableStorage> {
    /// Get a single value from the store by cloning the value.
    ///
    /// Returns `None` if there is no value for the specified key.
    pub fn get<D: schema::Singleton>(&self, _owner: Owner) -> Option<D::Value>
    where
        D::Value: Clone,
    {
        let storage = self.storage.read().unwrap();
        storage
            .get_singleton_value::<D>()
            .map_singleton_value(|v| D::Value::clone(D::from_value_ref(v)))
    }

    /// Get a single value from the store by cloning an `Arc`.
    ///
    /// Returns `None` if there is no value for the specified key. Panics if the value is not an `Arc`.
    pub fn get_arc<D: schema::ArcSingleton>(&self, _owner: Owner) -> Option<Arc<D::Value>> {
        let storage = self.storage.read().unwrap();
        storage
            .get_singleton_value::<D>()
            .map_singleton_value(|v| D::from_value_arc(v))
    }

    /// Get immutable access to a value in the store by reference.
    ///
    /// Returns `None` (and does not call `f`) if there is no value for the specified key.
    pub fn with<D: schema::Singleton, T>(
        &self,
        _owner: Owner,
        f: impl FnOnce(&D::Value) -> T,
    ) -> Option<T> {
        let storage = self.storage.read().unwrap();
        storage
            .get_singleton_value::<D>()
            .map_singleton_value(|v| f(D::from_value_ref(v)))
    }

    /// Insert a single value into the store.
    ///
    /// Returns the previous value if there is one, or `None` if there is no value for the specified key.
    // Question: do we need separate insert/update/upsert methods?
    pub fn insert<D: schema::Singleton>(
        &self,
        owner: Owner,
        value: D::ArgValue,
    ) -> Option<D::ArgValue> {
        let mut storage = self.storage.write().unwrap();
        let key = storage.hash_for_type::<D>();
        storage
            .singletons
            .insert(key, (owner, D::to_value(value)))
            .map_singleton_value(|v| D::from_value(v))
    }

    /// Get mutable access to a value in the store.
    ///
    /// Returns `None` (and does not call `f`) if there is no value for the specified key.
    pub fn mutate<D: schema::MutSingleton, T>(
        &self,
        _owner: Owner,
        mut f: impl FnMut(&mut D::Value) -> T,
    ) -> Option<T> {
        let mut storage = self.storage.write().unwrap();
        storage
            .get_singleton_value_mut::<D>()
            .map_singleton_value(|v| f(D::from_value_mut(v)))
    }

    /// Remove a single value from the store.
    ///
    /// Returns the previous value if there is one, or `None` if there is no value for the specified key.
    pub fn remove<D: schema::Singleton>(&self, _owner: Owner) -> Option<D::ArgValue> {
        let mut storage = self.storage.write().unwrap();
        let key = storage.hash_for_type::<D>();
        storage
            .singletons
            .remove(&key)
            .map_singleton_value(|v| D::from_value(v))
    }

    /// Remove a single value from the store while preserving ownership of the key/value.
    ///
    /// Can also be used to initialize a key/value with a key but without a value.
    ///
    /// Returns the previous value if there is one, or `None` if there is no value for the specified key.
    pub fn clear<D: schema::Singleton>(&self, owner: Owner) -> Option<D::ArgValue> {
        let mut storage = self.storage.write().unwrap();
        let key = storage.hash_for_type::<D>();
        storage
            .singletons
            .insert(key, (owner, SinValue::None))
            .map_singleton_value(|v| D::from_value(v))
    }

    /// Operate on tables of key/values in the store.
    ///
    /// Example:
    /// ```rust,ignore
    /// let value = store.table(OWNER).get(key).unwrap();
    /// ```
    pub fn table<'a, D: schema::TableDesc<Storage = TableStorage>>(
        &'a self,
        owner: Owner,
    ) -> KvTable<'a, TableStorage, D> {
        KvTable {
            store: self,
            owner,
            table: PhantomData,
        }
    }
}

/// Abstracts a table of key/values pairs in the store.
///
/// `KvTable` has no transactional semantics and only exists as a convenience for accessing
/// tabular data.
pub struct KvTable<
    'a,
    TableStorage: schema::GeneratedStorage,
    D: schema::TableDesc<Storage = TableStorage>,
> {
    store: &'a KvStore<TableStorage>,
    owner: Owner,
    table: PhantomData<D>,
}

impl<'a, TableStorage: schema::GeneratedStorage, D: schema::TableDesc<Storage = TableStorage>>
    KvTable<'a, TableStorage, D>
{
    /// Initialize a table by setting its owner.
    ///
    /// Calling this function is optional, a table can be used without initialization in which case,
    /// its owner is set to the owner specifed in the first write.
    ///
    /// Returns an error (containing the current owner of the table) if the table has already been
    /// initialized. In this case, the table will be in a consistent state and can be used as normal.
    pub fn init(&self) -> Result<()> {
        let mut storage = self.store.storage.write().unwrap();
        let table = D::get_table_mut(&mut storage.tables);
        match &table.owner {
            Some(owner) => Err(Error::AlreadyInit(owner)),
            None => {
                table.owner = Some(self.owner);
                Ok(())
            }
        }
    }

    /// Clear a table by removing all its KVs, but preserving ownership.
    pub fn clear(&self) {
        let mut storage = self.store.storage.write().unwrap();
        let table = D::get_table_mut(&mut storage.tables);
        table.data.clear();
    }

    /// Iterate all the key/value pairs in a table.
    pub fn iter(&'a self) -> impl Iterator<Item = (&'a D::Key, &'a D::Value)>
    where
        TableStorage: 'static,
        D: 'static,
    {
        let guard = self.store.storage.read().unwrap();
        TableIterator::<'a, RwLockReadGuard<'a, _>, TableStorage, D>::new(guard)
    }

    /// The number of key/value pairs in the table.
    pub fn len(&self) -> usize {
        let storage = self.store.storage.read().unwrap();
        let table = D::get_table(&storage.tables);
        table.data.len()
    }

    /// True if the table is empty.
    pub fn is_empty(&self) -> bool {
        let storage = self.store.storage.read().unwrap();
        let table = D::get_table(&storage.tables);
        table.data.is_empty()
    }

    /// Get a row of the table from the store by cloning the value.
    ///
    /// Returns `None` if there is no value for the specified key.
    pub fn get<Q>(&self, key: &Q) -> Option<D::Value>
    where
        D::Value: Clone,
        D::Key: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let storage = self.store.storage.read().unwrap();
        let table = D::get_table(&storage.tables);
        table.data.get(key).cloned()
    }

    /// Get immutable access to a row of the table in the store by reference.
    ///
    /// Returns `None` (and does not call `f`) if there is no value for the specified key.
    pub fn with<Q, T>(&self, f: impl FnOnce(&D::Value) -> T, key: &Q) -> Option<T>
    where
        D::Key: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let storage = self.store.storage.read().unwrap();
        let table = D::get_table(&storage.tables);
        let value = table.data.get(key)?;

        Some(f(value))
    }

    /// Insert a value into the table.
    ///
    /// Returns the previous value if there is one, or `None` if there is no value for the specified key.
    pub fn insert(&self, key: D::Key, value: D::Value) -> Option<D::Value> {
        let mut storage = self.store.storage.write().unwrap();
        let table = D::get_table_mut(&mut storage.tables);
        table.data.insert(key, value)
    }

    /// Get mutable access to a row of the table in the store in the store.
    ///
    /// Returns `None` (and does not call `f`) if there is no value for the specified key.
    pub fn mutate<Q, T>(&self, key: &Q, mut f: impl FnMut(&mut D::Value) -> T) -> Option<T>
    where
        D::Key: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let mut storage = self.store.storage.write().unwrap();
        let table = D::get_table_mut(&mut storage.tables);
        let value = table.data.get_mut(key)?;

        Some(f(value))
    }

    /// Remove a row from the table.
    ///
    /// Returns the previous value if there is one, or `None` if there is no value for the specified key.
    pub fn remove<Q>(&self, key: &Q) -> Option<D::Value>
    where
        D::Key: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let mut storage = self.store.storage.write().unwrap();
        let table = D::get_table_mut(&mut storage.tables);
        table.data.remove(key.borrow())
    }
}

//! KvStore transactional API.

use crate::{
    Error, KvStore, Owner, Result, TableIterator, schema,
    singleton::OptSingletonValue,
    storage::{SinValue, Storage},
};
use std::{
    borrow::Borrow,
    hash::Hash,
    marker::PhantomData,
    ops::Deref,
    sync::{Arc, RwLockReadGuard, RwLockWriteGuard, TryLockError},
};

impl<TableStorage: schema::GeneratedStorage> KvStore<TableStorage> {
    /// Start a transaction.
    ///
    /// Blocks until the store's global lock is available for write access.
    pub fn begin_transaction(&self, owner: Owner) -> Transaction<'_, TableStorage> {
        Transaction {
            guard: self.storage.write().unwrap(),
            owner,
        }
    }

    /// Start a transaction.
    ///
    /// Returns `None` if the store's global lock is unavailable for write access.
    pub fn try_begin_transaction(&self, owner: Owner) -> Option<Transaction<'_, TableStorage>> {
        let guard = match self.storage.try_write() {
            Ok(g) => g,
            Err(TryLockError::WouldBlock) => return None,
            Err(TryLockError::Poisoned(_)) => panic!(),
        };
        Some(Transaction { guard, owner })
    }

    /// Start a read-only transaction (i.e., only supports non-mutating access to the store, but
    /// all reads are guaranteed to be atomic).
    ///
    /// Blocks until the store's global lock is available for read access.
    pub fn begin_ro_transaction(&self, owner: Owner) -> RoTransaction<'_, TableStorage> {
        RoTransaction {
            guard: self.storage.read().unwrap(),
            _owner: owner,
        }
    }

    /// Start a read-only transaction (i.e., only supports non-mutating access to the store, but
    /// all reads are guaranteed to be atomic).
    ///
    /// Returns `None` if the store's global lock is unavailable for write access.
    pub fn try_begin_ro_transaction(
        &self,
        owner: Owner,
    ) -> Option<RoTransaction<'_, TableStorage>> {
        let guard = match self.storage.try_read() {
            Ok(g) => g,
            Err(TryLockError::WouldBlock) => return None,
            Err(TryLockError::Poisoned(_)) => panic!(),
        };
        Some(RoTransaction {
            guard,
            _owner: owner,
        })
    }

    // TODO single-table transactions?
}

/// TODO docs (no commit)
// TODO do we need to be able to abort a transaction?
pub struct Transaction<'a, TableStorage: schema::GeneratedStorage> {
    guard: RwLockWriteGuard<'a, Storage<TableStorage>>,
    owner: Owner,
}

impl<'a, TableStorage: schema::GeneratedStorage> Transaction<'a, TableStorage> {
    /// Get a single value from the store by cloning the value.
    ///
    /// Returns `None` if there is no value for the specified key.
    pub fn get<D: schema::Singleton>(&self) -> Option<D::Value>
    where
        D::Value: Clone,
    {
        self.guard
            .get_singleton_value(&D::KEY)
            .map_singleton_value(|v| D::Value::clone(D::from_value_ref(v)))
    }

    /// Get a single value from the store by cloning an `Arc`.
    ///
    /// Returns `None` if there is no value for the specified key. Panics if the value is not an `Arc`.
    pub fn get_arc<D: schema::ArcSingleton>(&self) -> Option<Arc<D::Value>> {
        self.guard
            .get_singleton_value(&D::KEY)
            .map_singleton_value(|v| D::from_value_arc(v))
    }

    /// Get immutable access to a value in the store by reference.
    ///
    /// Returns `None` (and does not call `f`) if there is no value for the specified key.
    pub fn with<D: schema::Singleton, T>(&self, f: impl FnOnce(&D::Value) -> T) -> Option<T> {
        self.guard
            .get_singleton_value(&D::KEY)
            .map_singleton_value(|v| f(D::from_value_ref(v)))
    }

    /// Insert a single value into the store.
    ///
    /// Returns the previous value if there is one, or `None` if there is no value for the specified key.
    // Question: do we need separate insert/update/upsert methods?
    pub fn insert<D: schema::Singleton>(&mut self, value: D::ArgValue) -> Option<D::ArgValue> {
        let storage = &mut self.guard;
        let key = storage.hash(&D::KEY);
        storage
            .singletons
            .insert(key, (self.owner, D::to_value(value)))
            .map_singleton_value(|v| D::from_value(v))
    }

    /// Get mutable access to a value in the store.
    ///
    /// Returns `None` (and does not call `f`) if there is no value for the specified key.
    pub fn mutate<D: schema::MutSingleton, T>(
        &mut self,
        mut f: impl FnMut(&mut D::Value) -> T,
    ) -> Option<T> {
        self.guard
            .get_singleton_value_mut(&D::KEY)
            .map_singleton_value(|v| f(D::from_value_mut(v)))
    }

    /// Remove a single value from the store.
    ///
    /// Returns the previous value if there is one, or `None` if there is no value for the specified key.
    pub fn remove<D: schema::Singleton>(&mut self) -> Option<D::ArgValue> {
        let storage = &mut self.guard;
        let key = storage.hash(&D::KEY);
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
    pub fn clear<D: schema::Singleton>(&mut self) -> Option<D::ArgValue> {
        let storage = &mut self.guard;
        let key = storage.hash(&D::KEY);
        storage
            .singletons
            .insert(key, (self.owner, SinValue::None))
            .map_singleton_value(|v| D::from_value(v))
    }

    /// Operate on tables of key/values in the store.
    ///
    /// Example:
    /// ```rust,ignore
    /// let value = store.table(OWNER).get(key).unwrap();
    /// ```
    pub fn table<D: schema::TableDesc<TableStorage>>(
        &'a mut self,
    ) -> KvTableTransactional<'a, TableStorage, D> {
        KvTableTransactional {
            txn: self,
            table: PhantomData,
        }
    }
}

/// Abstracts a table of key/values pairs in the store.
///
/// `KvTable` has no transactional semantics and only exists as a convenience for accessing
/// tabular data.
pub struct KvTableTransactional<
    'a,
    TableStorage: schema::GeneratedStorage,
    D: schema::TableDesc<TableStorage>,
> {
    txn: &'a mut Transaction<'a, TableStorage>,
    table: PhantomData<D>,
}

impl<'a, TableStorage: schema::GeneratedStorage, D: schema::TableDesc<TableStorage>>
    KvTableTransactional<'a, TableStorage, D>
{
    /// Initialize a table by setting its owner.
    ///
    /// Calling this function is optional, a table can be used without initialization in which case,
    /// its owner is set to the owner specifed in the first write.
    ///
    /// Returns an error (containing the current owner of the table) if the table has already been
    /// initialized. In this case, the table will be in a consistent state and can be used as normal.
    pub fn init(&mut self) -> Result<()> {
        let storage = &mut self.txn.guard;
        let table = D::get_table_mut(&mut storage.tables);
        match &table.owner {
            Some(owner) => Err(Error::AlreadyInit(owner)),
            None => {
                table.owner = Some(self.txn.owner);
                Ok(())
            }
        }
    }

    /// Clear a table by removing all its KVs, but preserving ownership.
    pub fn clear(&mut self) {
        let storage = &mut self.txn.guard;
        let table = D::get_table_mut(&mut storage.tables);
        table.data.clear();
    }

    /// Iterate all the key/value pairs in a table.
    pub fn iter(&'a self) -> impl Iterator<Item = (&'a D::Key, &'a D::Value)>
    where
        TableStorage: 'static,
        D: 'static,
    {
        let guard = &self.txn.guard;
        TableIterator::<'a, RefWriteGuard<'a, _>, TableStorage, D>::new(RefWriteGuard(guard))
    }

    /// The number of key/value pairs in the table.
    pub fn len(&self) -> usize {
        let storage = &self.txn.guard;
        let table = D::get_table(&storage.tables);
        table.data.len()
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
        let storage = &self.txn.guard;
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
        let storage = &self.txn.guard;
        let table = D::get_table(&storage.tables);
        let value = table.data.get(key)?;

        Some(f(value))
    }

    /// Insert a value into the table.
    ///
    /// Returns the previous value if there is one, or `None` if there is no value for the specified key.
    pub fn insert(&mut self, key: D::Key, value: D::Value) -> Option<D::Value> {
        let storage = &mut self.txn.guard;
        let table = D::get_table_mut(&mut storage.tables);
        table.data.insert(key, value)
    }

    /// Get mutable access to a row of the table in the store in the store.
    ///
    /// Returns `None` (and does not call `f`) if there is no value for the specified key.
    pub fn mutate<Q, T>(&mut self, key: &Q, mut f: impl FnMut(&mut D::Value) -> T) -> Option<T>
    where
        D::Key: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let storage = &mut self.txn.guard;
        let table = D::get_table_mut(&mut storage.tables);
        let value = table.data.get_mut(key)?;

        Some(f(value))
    }

    /// Remove a row from the table.
    ///
    /// Returns the previous value if there is one, or `None` if there is no value for the specified key.
    pub fn remove<Q>(&mut self, key: &Q) -> Option<D::Value>
    where
        D::Key: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let storage = &mut self.txn.guard;
        let table = D::get_table_mut(&mut storage.tables);
        table.data.remove(key.borrow())
    }
}

struct RefWriteGuard<'a, T>(&'a RwLockWriteGuard<'a, T>);

impl<'a, T> Deref for RefWriteGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

/// TODO docs
pub struct RoTransaction<'a, TableStorage: schema::GeneratedStorage> {
    guard: RwLockReadGuard<'a, Storage<TableStorage>>,
    _owner: Owner,
}

impl<TableStorage: schema::GeneratedStorage> RoTransaction<'_, TableStorage> {
    /// Get a single value from the store by cloning the value.
    ///
    /// Returns `None` if there is no value for the specified key.
    pub fn get<D: schema::Singleton>(&self) -> Option<D::Value>
    where
        D::Value: Clone,
    {
        self.guard
            .get_singleton_value(&D::KEY)
            .map_singleton_value(|v| D::Value::clone(D::from_value_ref(v)))
    }

    /// Get a single value from the store by cloning an `Arc`.
    ///
    /// Returns `None` if there is no value for the specified key. Panics if the value is not an `Arc`.
    pub fn get_arc<D: schema::ArcSingleton>(&self) -> Option<Arc<D::Value>> {
        self.guard
            .get_singleton_value(&D::KEY)
            .map_singleton_value(|v| D::from_value_arc(v))
    }

    /// Get immutable access to a value in the store by reference.
    ///
    /// Returns `None` (and does not call `f`) if there is no value for the specified key.
    pub fn with<D: schema::Singleton, T>(&self, f: impl FnOnce(&D::Value) -> T) -> Option<T> {
        self.guard
            .get_singleton_value(&D::KEY)
            .map_singleton_value(|v| f(D::from_value_ref(v)))
    }

    /// Operate on tables of key/values in the store.
    ///
    /// Example:
    /// ```rust,ignore
    /// let value = store.table(OWNER).get(key).unwrap();
    /// ```
    pub fn table<'a, D: schema::TableDesc<TableStorage>>(
        &'a self,
    ) -> KvTableRoTransactional<'a, TableStorage, D> {
        KvTableRoTransactional {
            txn: self,
            table: PhantomData,
        }
    }
}

/// Abstracts a table of key/values pairs in the store.
///
/// `KvTable` has no transactional semantics and only exists as a convenience for accessing
/// tabular data.
pub struct KvTableRoTransactional<
    'a,
    TableStorage: schema::GeneratedStorage,
    D: schema::TableDesc<TableStorage>,
> {
    txn: &'a RoTransaction<'a, TableStorage>,
    table: PhantomData<D>,
}

impl<'a, TableStorage: schema::GeneratedStorage, D: schema::TableDesc<TableStorage>>
    KvTableRoTransactional<'a, TableStorage, D>
{
    /// Iterate all the key/value pairs in a table.
    pub fn iter(&self) -> impl Iterator<Item = (&'a D::Key, &'a D::Value)>
    where
        TableStorage: 'static,
        D: 'static,
    {
        let guard = &self.txn.guard;
        TableIterator::<'a, RefReadGuard<'a, _>, TableStorage, D>::new(RefReadGuard(guard))
    }

    /// The number of key/value pairs in the table.
    pub fn len(&self) -> usize {
        let storage = &self.txn.guard;
        let table = D::get_table(&storage.tables);
        table.data.len()
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
        let storage = &self.txn.guard;
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
        let storage = &self.txn.guard;
        let table = D::get_table(&storage.tables);
        let value = table.data.get(key)?;

        Some(f(value))
    }
}

struct RefReadGuard<'a, T>(&'a RwLockReadGuard<'a, T>);

impl<'a, T> Deref for RefReadGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

//! KvStore transactional API.

use crate::{
    Error, KvStore, Owner, Result, TableIterator, schema,
    singleton::{OptSingletonValue, assert_owner},
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

/// A read/write transaction over a [`KvStore`]`.
///
/// Create a transaction by calling [`KvStore::begin_transaction`] or [`KvStore::try_begin_transaction`].
/// A transaction holds a write lock on the whole store while it is active so ensure that code within
/// a transaction is relatively quick to execute and that you drop transactions as soon as possible
/// ([`Transaction::commit`] can be used for this).
///
/// A transaction must not be kept alive over an `await` point. This can lead to deadlock.
// TODO do we need to be able to abort a transaction?
pub struct Transaction<'a, TableStorage: schema::GeneratedStorage> {
    guard: RwLockWriteGuard<'a, Storage<TableStorage>>,
    owner: Owner,
}

impl<'a, TableStorage: schema::GeneratedStorage> Transaction<'a, TableStorage> {
    /// Commit this transaction.
    ///
    /// This simply moves and drops the `Transaction` object. It is optional to call and currently
    /// always succeeds. You can use this method to release the transaction's lock on the store
    /// without needing an explicit scope.
    pub fn commit(self) -> Result<()> {
        // drop `self` to release the lock.
        Ok(())
    }

    /// Get a single value from the store by cloning the value.
    ///
    /// Returns `None` if there is no value for the specified key.
    pub fn get<D: schema::Singleton>(&self) -> Option<D::Value>
    where
        D::Value: Clone,
    {
        let storage = &self.guard;
        let key = storage.hash_for_type::<D>();
        storage
            .get_singleton_value(key)
            .map_singleton_value(|v| D::Value::clone(D::from_value_ref(v)))
    }

    /// Get a single value from the store by cloning an `Arc`.
    ///
    /// Returns `None` if there is no value for the specified key. Panics if the value is not an `Arc`.
    pub fn get_arc<D: schema::ArcSingleton>(&self) -> Option<Arc<D::Value>> {
        let storage = &self.guard;
        let key = storage.hash_for_type::<D>();
        storage
            .get_singleton_value(key)
            .map_singleton_value(|v| D::from_value_arc(v))
    }

    /// Get immutable access to a value in the store by reference.
    ///
    /// Returns `None` (and does not call `f`) if there is no value for the specified key.
    pub fn with<D: schema::Singleton, T>(&self, f: impl FnOnce(&D::Value) -> T) -> Option<T> {
        let storage = &self.guard;
        let key = storage.hash_for_type::<D>();
        storage
            .get_singleton_value(key)
            .map_singleton_value(|v| f(D::from_value_ref(v)))
    }

    /// Insert a single value into the store.
    ///
    /// Returns the previous value if there is one, or `None` if there is no value for the specified key.
    // Question: do we need separate insert/update/upsert methods?
    pub fn insert<D: schema::Singleton>(&mut self, value: D::ArgValue) -> Option<D::ArgValue> {
        let storage = &mut self.guard;
        let key = storage.hash_for_type::<D>();
        assert_owner(self.owner, key, storage);
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
        let storage = &mut self.guard;
        let key = storage.hash_for_type::<D>();
        assert_owner(self.owner, key, storage);
        storage
            .get_singleton_value_mut(key)
            .map_singleton_value(|v| f(D::from_value_mut(v)))
    }

    /// Remove a single value from the store.
    ///
    /// Returns the previous value if there is one, or `None` if there is no value for the specified key.
    pub fn remove<D: schema::Singleton>(&mut self) -> Option<D::ArgValue> {
        let storage = &mut self.guard;
        let key = storage.hash_for_type::<D>();
        assert_owner(self.owner, key, storage);
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
        let key = storage.hash_for_type::<D>();
        assert_owner(self.owner, key, storage);
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
    pub fn table<'t, D: schema::TableDesc<Storage = TableStorage>>(
        &'t mut self,
    ) -> KvTableTransactional<'a, 't, TableStorage, D> {
        KvTableTransactional {
            txn: self,
            table: PhantomData,
        }
    }
}

/// Abstracts a table of key/values pairs in the store accessed as part of a transaction.
pub struct KvTableTransactional<
    'a,
    't,
    TableStorage: schema::GeneratedStorage,
    D: schema::TableDesc<Storage = TableStorage>,
> {
    txn: &'t mut Transaction<'a, TableStorage>,
    table: PhantomData<D>,
}

impl<'a, 't, TableStorage: schema::GeneratedStorage, D: schema::TableDesc<Storage = TableStorage>>
    KvTableTransactional<'a, 't, TableStorage, D>
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
        table.assert_or_set_owner(self.txn.owner);
        table.data.clear();
    }

    /// Iterate all the key/value pairs in a table.
    pub fn iter<'s>(&'s self) -> impl Iterator<Item = (&'s D::Key, &'s D::Value)> + 's
    where
        TableStorage: 'static,
        D: 'static,
    {
        let guard = &self.txn.guard;
        TableIterator::<'s, RefWriteGuard<'s, 'a, _>, TableStorage, D>::new(RefWriteGuard(guard))
    }

    /// The number of key/value pairs in the table.
    pub fn len(&self) -> usize {
        let storage = &self.txn.guard;
        let table = D::get_table(&storage.tables);
        table.data.len()
    }

    /// True if the table is empty.
    pub fn is_empty(&self) -> bool {
        let storage = &self.txn.guard;
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
        table.assert_or_set_owner(self.txn.owner);
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
        table.assert_owner(self.txn.owner);
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
        table.assert_owner(self.txn.owner);
        table.data.remove(key.borrow())
    }
}

/// Helper type for using a reference to a [`RwLockWriteGuard`] as a generic argument to
/// [`TableIterator`]. Required because checking trait bounds does not take into account
/// transitivity of `Deref`.
struct RefWriteGuard<'r, 'a, T>(&'r RwLockWriteGuard<'a, T>);

impl<'r, 'a, T> Deref for RefWriteGuard<'r, 'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

/// A read-only transaction over a [`KvStore`]`.
///
/// Create a read-only transaction by calling [`KvStore::begin_ro_transaction`] or [`KvStore::try_begin_ro_transaction`].
/// A read-only transaction holds a read lock on the whole store while it is active so ensure that
/// code within a transaction is relatively quick to execute and that you drop transactionss as soon
/// as possible.
///
/// A transaction must not be kept alive over an `await` point. This can lead to deadlock.
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
        let storage = &self.guard;
        let key = storage.hash_for_type::<D>();
        storage
            .get_singleton_value(key)
            .map_singleton_value(|v| D::Value::clone(D::from_value_ref(v)))
    }

    /// Get a single value from the store by cloning an `Arc`.
    ///
    /// Returns `None` if there is no value for the specified key. Panics if the value is not an `Arc`.
    pub fn get_arc<D: schema::ArcSingleton>(&self) -> Option<Arc<D::Value>> {
        let storage = &self.guard;
        let key = storage.hash_for_type::<D>();
        storage
            .get_singleton_value(key)
            .map_singleton_value(|v| D::from_value_arc(v))
    }

    /// Get immutable access to a value in the store by reference.
    ///
    /// Returns `None` (and does not call `f`) if there is no value for the specified key.
    pub fn with<D: schema::Singleton, T>(&self, f: impl FnOnce(&D::Value) -> T) -> Option<T> {
        let storage = &self.guard;
        let key = storage.hash_for_type::<D>();
        storage
            .get_singleton_value(key)
            .map_singleton_value(|v| f(D::from_value_ref(v)))
    }

    /// Operate on tables of key/values in the store.
    ///
    /// Example:
    /// ```rust,ignore
    /// let value = store.table(OWNER).get(key).unwrap();
    /// ```
    pub fn table<'a, D: schema::TableDesc<Storage = TableStorage>>(
        &'a self,
    ) -> KvTableRoTransactional<'a, TableStorage, D> {
        KvTableRoTransactional {
            txn: self,
            table: PhantomData,
        }
    }
}

/// Abstracts a table of key/values pairs in the store as part of a read-only transaction.
pub struct KvTableRoTransactional<
    'a,
    TableStorage: schema::GeneratedStorage,
    D: schema::TableDesc<Storage = TableStorage>,
> {
    txn: &'a RoTransaction<'a, TableStorage>,
    table: PhantomData<D>,
}

impl<'a, TableStorage: schema::GeneratedStorage, D: schema::TableDesc<Storage = TableStorage>>
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

    /// True if the table is empty.
    pub fn is_empty(&self) -> bool {
        let storage = &self.txn.guard;
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

/// Helper type for using a reference to a [`RwLockReadGuard`] as a generic argument to
/// [`TableIterator`]. Required because checking trait bounds does not take into account
/// transitivity of `Deref`.
struct RefReadGuard<'a, T>(&'a RwLockReadGuard<'a, T>);

impl<'a, T> Deref for RefReadGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

#[cfg(test)]
mod test {
    use crate::{Error, singleton, tables};
    use std::any::Any;
    use std::sync::Arc;

    singleton!(Count(u64));
    singleton!(Shared(String as Arc));

    tables!(Items(&'static str => String), Counters(u32 => u64));

    const OWNER: &str = "owner";
    #[cfg(debug_assertions)]
    const OTHER: &str = "other";

    #[test]
    fn begin_transaction_works() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        txn.insert::<Count>(42);
        assert_eq!(txn.get::<Count>(), Some(42));
    }

    #[test]
    fn try_begin_transaction_returns_some_when_unlocked() {
        let store = KvStore::new();
        assert!(store.try_begin_transaction(OWNER).is_some());
    }

    #[test]
    fn begin_ro_transaction_works() {
        let store = KvStore::new();
        store.insert::<Count>(OWNER, 7);
        let txn = store.begin_ro_transaction(OWNER);
        assert_eq!(txn.get::<Count>(), Some(7));
    }

    #[test]
    fn try_begin_ro_transaction_returns_some_when_unlocked() {
        let store = KvStore::new();
        assert!(store.try_begin_ro_transaction(OWNER).is_some());
    }

    #[test]
    fn txn_get_returns_none_when_absent() {
        let store = KvStore::new();
        let txn = store.begin_transaction(OWNER);
        assert!(txn.get::<Count>().is_none());
    }

    #[test]
    fn txn_get_returns_value_inserted_in_same_txn() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        txn.insert::<Count>(42);
        assert_eq!(txn.get::<Count>(), Some(42));
    }

    #[test]
    fn txn_get_returns_value_inserted_before_txn() {
        let store = KvStore::new();
        store.insert::<Count>(OWNER, 5);
        let txn = store.begin_transaction(OWNER);
        assert_eq!(txn.get::<Count>(), Some(5));
    }

    #[test]
    fn txn_get_returns_none_after_clear_in_same_txn() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        txn.insert::<Count>(1);
        txn.clear::<Count>();
        assert!(txn.get::<Count>().is_none());
    }

    #[test]
    fn txn_get_arc_returns_none_when_absent() {
        let store = KvStore::new();
        let txn = store.begin_transaction(OWNER);
        assert!(txn.get_arc::<Shared>().is_none());
    }

    #[test]
    fn txn_get_arc_returns_arc_after_insert() {
        let store = KvStore::new();
        store.insert::<Shared>(OWNER, Arc::new("hello".to_owned()));
        let txn = store.begin_transaction(OWNER);
        let arc = txn.get_arc::<Shared>().unwrap();
        assert_eq!(*arc, "hello");
    }

    #[test]
    fn txn_get_arc_shares_allocation() {
        let store = KvStore::new();
        store.insert::<Shared>(OWNER, Arc::new("hello".to_owned()));
        let txn = store.begin_transaction(OWNER);
        let arc1 = txn.get_arc::<Shared>().unwrap();
        let arc2 = txn.get_arc::<Shared>().unwrap();
        assert!(Arc::ptr_eq(&arc1, &arc2));
    }

    #[test]
    fn txn_with_returns_none_and_does_not_call_f_when_absent() {
        let store = KvStore::new();
        let txn = store.begin_transaction(OWNER);
        let mut called = false;
        let result = txn.with::<Count, ()>(|_| {
            called = true;
        });
        assert!(result.is_none());
        assert!(!called);
    }

    #[test]
    fn txn_with_returns_result_of_f() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        txn.insert::<Count>(5);
        assert_eq!(txn.with::<Count, _>(|v| v * 2), Some(10));
    }

    #[test]
    fn txn_insert_returns_none_on_first() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        assert!(txn.insert::<Count>(1).is_none());
    }

    #[test]
    fn txn_insert_returns_previous_value() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        txn.insert::<Count>(1);
        assert_eq!(txn.insert::<Count>(2), Some(1));
    }

    #[test]
    fn txn_insert_over_tombstone_returns_none() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        txn.insert::<Count>(1);
        txn.clear::<Count>();
        assert!(txn.insert::<Count>(2).is_none());
    }

    #[test]
    fn txn_mutate_returns_none_and_does_not_call_f_when_absent() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        let mut called = false;
        let result = txn.mutate::<Count, ()>(|_| {
            called = true;
        });
        assert!(result.is_none());
        assert!(!called);
    }

    #[test]
    fn txn_mutate_modifies_value_in_place() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        txn.insert::<Count>(10);
        assert_eq!(
            txn.mutate::<Count, _>(|v| {
                *v += 5;
                *v
            }),
            Some(15)
        );
        assert_eq!(txn.get::<Count>(), Some(15));
    }

    #[test]
    fn txn_remove_returns_none_when_absent() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        assert!(txn.remove::<Count>().is_none());
    }

    #[test]
    fn txn_remove_returns_previous_value() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        txn.insert::<Count>(7);
        assert_eq!(txn.remove::<Count>(), Some(7));
    }

    #[test]
    fn txn_remove_makes_get_return_none() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        txn.insert::<Count>(1);
        txn.remove::<Count>();
        assert!(txn.get::<Count>().is_none());
    }

    #[test]
    fn txn_clear_returns_none_when_absent() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        assert!(txn.clear::<Count>().is_none());
    }

    #[test]
    fn txn_clear_returns_previous_value() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        txn.insert::<Count>(3);
        assert_eq!(txn.clear::<Count>(), Some(3));
    }

    #[test]
    fn txn_clear_makes_get_return_none() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        txn.insert::<Count>(1);
        txn.clear::<Count>();
        assert!(txn.get::<Count>().is_none());
    }

    #[test]
    fn txn_double_clear_returns_none() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        txn.insert::<Count>(1);
        txn.clear::<Count>();
        assert!(txn.clear::<Count>().is_none());
    }

    #[test]
    fn txn_writes_visible_after_drop() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        txn.insert::<Count>(42);
        txn.commit().unwrap();

        assert_eq!(store.get::<Count>(OWNER), Some(42));
    }

    #[test]
    fn txn_table_writes_visible_after_drop() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        txn.table::<Items>().insert("k", "v".to_owned());
        txn.commit().unwrap();

        assert_eq!(store.table::<Items>(OWNER).get("k"), Some("v".to_owned()));
    }

    #[test]
    fn txn_mutate_visible_after_drop() {
        let store = KvStore::new();
        store.insert::<Count>(OWNER, 1);
        let mut txn = store.begin_transaction(OWNER);
        txn.mutate::<Count, ()>(|v| *v = 100);
        txn.commit().unwrap();

        assert_eq!(store.get::<Count>(OWNER), Some(100));
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "Ownership violation")]
    fn txn_insert_wrong_owner_panics() {
        let store = KvStore::new();
        store.insert::<Count>(OWNER, 1);
        let mut txn = store.begin_transaction(OTHER);
        txn.insert::<Count>(5);
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "Ownership violation")]
    fn txn_mutate_wrong_owner_panics() {
        let store = KvStore::new();
        store.insert::<Count>(OWNER, 1);
        let mut txn = store.begin_transaction(OTHER);
        txn.mutate::<Count, ()>(|_| {});
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "Ownership violation")]
    fn txn_remove_wrong_owner_panics() {
        let store = KvStore::new();
        store.insert::<Count>(OWNER, 1);
        let mut txn = store.begin_transaction(OTHER);
        txn.remove::<Count>();
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "Ownership violation")]
    fn txn_clear_wrong_owner_panics() {
        let store = KvStore::new();
        store.insert::<Count>(OWNER, 1);
        let mut txn = store.begin_transaction(OTHER);
        txn.clear::<Count>();
    }

    #[test]
    fn txn_table_init_succeeds() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        let mut table = txn.table::<Items>();
        assert!(table.init().is_ok());
    }

    #[test]
    fn txn_table_init_second_call_err() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        let mut table = txn.table::<Items>();
        table.init().unwrap();
        let err = table.init().unwrap_err();
        assert!(matches!(err, Error::AlreadyInit(o) if o == OWNER));
    }

    #[test]
    fn txn_table_get_returns_none_when_absent() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        let table = txn.table::<Items>();
        assert!(table.get("missing").is_none());
    }

    #[test]
    fn txn_table_insert_returns_none_on_first() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        let mut table = txn.table::<Items>();
        assert!(table.insert("k", "v".to_owned()).is_none());
    }

    #[test]
    fn txn_table_insert_returns_previous() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        let mut table = txn.table::<Items>();
        table.insert("k", "v1".to_owned());
        assert_eq!(table.insert("k", "v2".to_owned()), Some("v1".to_owned()));
    }

    #[test]
    fn txn_table_get_returns_value_after_insert() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        let mut table = txn.table::<Items>();
        table.insert("k", "val".to_owned());
        assert_eq!(table.get("k"), Some("val".to_owned()));
    }

    #[test]
    fn txn_table_with_returns_none_and_does_not_call_f_when_absent() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        let table = txn.table::<Items>();
        let mut called = false;
        let result = table.with(
            |_| {
                called = true;
            },
            "missing",
        );
        assert!(result.is_none());
        assert!(!called);
    }

    #[test]
    fn txn_table_with_returns_some_after_insert() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        let mut table = txn.table::<Items>();
        table.insert("k", "val".to_owned());
        assert_eq!(table.with(|s| s.len(), "k"), Some(3));
    }

    #[test]
    fn txn_table_mutate_returns_none_when_absent() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        let mut table = txn.table::<Items>();
        table.init().unwrap();
        assert!(table.mutate("missing", |v| v.len()).is_none());
    }

    #[test]
    fn txn_table_mutate_modifies_value() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        let mut table = txn.table::<Items>();
        table.insert("k", "hello".to_owned());
        table.mutate("k", |v| v.push('!'));
        assert_eq!(table.get("k"), Some("hello!".to_owned()));
    }

    #[test]
    fn txn_table_remove_returns_none_when_absent() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        let mut table = txn.table::<Items>();
        assert!(table.remove("missing").is_none());
    }

    #[test]
    fn txn_table_remove_returns_previous_value() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        let mut table = txn.table::<Items>();
        table.insert("k", "v".to_owned());
        assert_eq!(table.remove("k"), Some("v".to_owned()));
    }

    #[test]
    fn txn_table_remove_makes_get_return_none() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        let mut table = txn.table::<Items>();
        table.insert("k", "v".to_owned());
        table.remove("k");
        assert!(table.get("k").is_none());
    }

    #[test]
    fn txn_table_clear_removes_all_rows() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        let mut table = txn.table::<Items>();
        table.insert("a", "alpha".to_owned());
        table.insert("b", "beta".to_owned());
        table.insert("c", "gamma".to_owned());
        table.clear();
        assert!(table.is_empty());
    }

    #[test]
    fn txn_table_is_empty_on_fresh_store() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        let table = txn.table::<Items>();
        assert!(table.is_empty());
    }

    #[test]
    fn txn_table_len_reflects_inserts() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        let mut table = txn.table::<Items>();
        table.insert("a", "alpha".to_owned());
        table.insert("b", "beta".to_owned());
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn txn_table_iter_empty() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        let table = txn.table::<Items>();
        assert_eq!(table.iter().count(), 0);
    }

    #[test]
    fn txn_table_iter_yields_inserted_rows() {
        let store = KvStore::new();
        let mut txn = store.begin_transaction(OWNER);
        let mut table = txn.table::<Items>();
        table.insert("a", "alpha".to_owned());
        table.insert("b", "beta".to_owned());
        let mut items: Vec<_> = table.iter().map(|(&k, v)| (k, v.clone())).collect();
        items.sort();
        assert_eq!(
            items,
            vec![("a", "alpha".to_owned()), ("b", "beta".to_owned())]
        );
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "Ownership violation")]
    fn txn_table_insert_wrong_owner_panics() {
        let store = KvStore::new();
        store.table::<Items>(OWNER).init().unwrap();
        let mut txn = store.begin_transaction(OTHER);
        txn.table::<Items>().insert("k", "v".to_owned());
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "Ownership violation")]
    fn txn_table_mutate_wrong_owner_panics() {
        let store = KvStore::new();
        store.table::<Items>(OWNER).init().unwrap();
        let mut txn = store.begin_transaction(OTHER);
        txn.table::<Items>().mutate("k", |v: &mut String| v.len());
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "Ownership violation")]
    fn txn_table_remove_wrong_owner_panics() {
        let store = KvStore::new();
        store.table::<Items>(OWNER).init().unwrap();
        let mut txn = store.begin_transaction(OTHER);
        txn.table::<Items>().remove("k");
    }

    #[test]
    fn ro_txn_get_returns_none_when_absent() {
        let store = KvStore::new();
        let txn = store.begin_ro_transaction(OWNER);
        assert!(txn.get::<Count>().is_none());
    }

    #[test]
    fn ro_txn_get_returns_value_inserted_before_txn() {
        let store = KvStore::new();
        store.insert::<Count>(OWNER, 42);
        let txn = store.begin_ro_transaction(OWNER);
        assert_eq!(txn.get::<Count>(), Some(42));
    }

    #[test]
    fn ro_txn_get_arc_returns_none_when_absent() {
        let store = KvStore::new();
        let txn = store.begin_ro_transaction(OWNER);
        assert!(txn.get_arc::<Shared>().is_none());
    }

    #[test]
    fn ro_txn_get_arc_returns_arc() {
        let store = KvStore::new();
        store.insert::<Shared>(OWNER, Arc::new("hello".to_owned()));
        let txn = store.begin_ro_transaction(OWNER);
        let arc = txn.get_arc::<Shared>().unwrap();
        assert_eq!(*arc, "hello");
    }

    #[test]
    fn ro_txn_with_returns_none_and_does_not_call_f_when_absent() {
        let store = KvStore::new();
        let txn = store.begin_ro_transaction(OWNER);
        let mut called = false;
        let result = txn.with::<Count, ()>(|_| {
            called = true;
        });
        assert!(result.is_none());
        assert!(!called);
    }

    #[test]
    fn ro_txn_with_returns_some_after_insert() {
        let store = KvStore::new();
        store.insert::<Count>(OWNER, 4);
        let txn = store.begin_ro_transaction(OWNER);
        assert_eq!(txn.with::<Count, _>(|v| v * 2), Some(8));
    }

    #[test]
    fn ro_txn_table_get_returns_none_when_absent() {
        let store = KvStore::new();
        let txn = store.begin_ro_transaction(OWNER);
        let table = txn.table::<Items>();
        assert!(table.get("missing").is_none());
    }

    #[test]
    fn ro_txn_table_get_returns_value_inserted_before_txn() {
        let store = KvStore::new();
        store.table::<Items>(OWNER).insert("k", "val".to_owned());
        let txn = store.begin_ro_transaction(OWNER);
        let table = txn.table::<Items>();
        assert_eq!(table.get("k"), Some("val".to_owned()));
    }

    #[test]
    fn ro_txn_table_with_returns_none_and_does_not_call_f_when_absent() {
        let store = KvStore::new();
        let txn = store.begin_ro_transaction(OWNER);
        let table = txn.table::<Items>();
        let mut called = false;
        let result = table.with(
            |_| {
                called = true;
            },
            "missing",
        );
        assert!(result.is_none());
        assert!(!called);
    }

    #[test]
    fn ro_txn_table_with_returns_some() {
        let store = KvStore::new();
        store.table::<Items>(OWNER).insert("k", "val".to_owned());
        let txn = store.begin_ro_transaction(OWNER);
        let table = txn.table::<Items>();
        assert_eq!(table.with(|s| s.len(), "k"), Some(3));
    }

    #[test]
    fn ro_txn_table_len_zero_when_empty() {
        let store = KvStore::new();
        let txn = store.begin_ro_transaction(OWNER);
        let table = txn.table::<Items>();
        assert_eq!(table.len(), 0);
    }

    #[test]
    fn ro_txn_table_len_reflects_pre_txn_inserts() {
        let store = KvStore::new();
        store.table::<Items>(OWNER).insert("a", "alpha".to_owned());
        store.table::<Items>(OWNER).insert("b", "beta".to_owned());
        let txn = store.begin_ro_transaction(OWNER);
        let table = txn.table::<Items>();
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn ro_txn_table_is_empty_true_when_no_rows() {
        let store = KvStore::new();
        let txn = store.begin_ro_transaction(OWNER);
        let table = txn.table::<Items>();
        assert!(table.is_empty());
    }

    #[test]
    fn ro_txn_table_is_empty_false_after_inserts() {
        let store = KvStore::new();
        store.table::<Items>(OWNER).insert("k", "v".to_owned());
        let txn = store.begin_ro_transaction(OWNER);
        let table = txn.table::<Items>();
        assert!(!table.is_empty());
    }

    #[test]
    fn ro_txn_table_iter_empty() {
        let store = KvStore::new();
        let txn = store.begin_ro_transaction(OWNER);
        let table = txn.table::<Items>();
        assert_eq!(table.iter().count(), 0);
    }

    #[test]
    fn ro_txn_table_iter_yields_pre_txn_rows() {
        let store = KvStore::new();
        store.table::<Items>(OWNER).insert("a", "alpha".to_owned());
        store.table::<Items>(OWNER).insert("b", "beta".to_owned());
        let txn = store.begin_ro_transaction(OWNER);
        let table = txn.table::<Items>();
        let mut items: Vec<_> = table.iter().map(|(&k, v)| (k, v.clone())).collect();
        items.sort();
        assert_eq!(
            items,
            vec![("a", "alpha".to_owned()), ("b", "beta".to_owned())]
        );
    }
}

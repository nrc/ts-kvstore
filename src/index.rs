use crate::{
    Error, KvStore, Owner, Result,
    iter::IndexIterator,
    schema::{self, IndexDesc, IndexStorage, TableDesc},
};
use std::{borrow::Borrow, hash::Hash, marker::PhantomData, sync::RwLockReadGuard};

pub struct KvTableIndex<
    'a,
    TableStorage: schema::GeneratedStorage,
    D: IndexDesc<Storage = TableStorage, BaseTable = B>,
    B: TableDesc<Storage = TableStorage>,
> {
    pub store: &'a KvStore<TableStorage>,
    pub owner: Owner,
    pub index: PhantomData<D>,
    pub base: PhantomData<B>,
}

impl<
    'a,
    TableStorage: schema::GeneratedStorage,
    D: IndexDesc<Storage = TableStorage, BaseTable = B, Value = B::Key>,
    B: TableDesc<Storage = TableStorage>,
> KvTableIndex<'a, TableStorage, D, B>
{
    /// Initialize the base table by setting its owner (indexes don't have an owner).
    ///
    /// Calling this function is optional, a table can be used without initialization in which case,
    /// its owner is set to the owner specifed in the first write.
    ///
    /// Returns an error (containing the current owner of the table) if the table has already been
    /// initialized. In this case, the table will be in a consistent state and can be used as normal.
    pub fn init(&self) -> Result<()> {
        let mut storage = self.store.storage.write().unwrap();
        let base = D::BaseTable::get_table_mut(&mut storage.tables);
        match &base.owner {
            Some(owner) => Err(Error::AlreadyInit(owner)),
            None => {
                base.owner = Some(self.owner);
                Ok(())
            }
        }
    }

    /// Iterate all the keys in the index and value in the base table.
    pub fn iter(&'a self) -> impl Iterator<Item = (&'a D::Key, &'a B::Value)>
    where
        TableStorage: 'static,
        D: 'static,
        B: 'static,
    {
        let guard = self.store.storage.read().unwrap();
        IndexIterator::<'a, RwLockReadGuard<'a, _>, TableStorage, D, B>::new(guard)
    }

    /// Clear the base table by removing all its KVs, but preserving ownership.
    pub fn clear(&self) {
        let mut storage = self.store.storage.write().unwrap();
        let base: &mut crate::storage::Table<B, <B as TableDesc>::Indexes> =
            B::get_table_mut(&mut storage.tables);
        base.assert_or_set_owner(self.owner);
        base.indexes.clear();
        base.data.clear();
    }

    /// The number of key/value pairs in the base table.
    pub fn len(&self) -> usize {
        let storage = self.store.storage.read().unwrap();
        let base = B::get_table(&storage.tables);
        base.data.len()
    }

    /// True if the table is empty.
    pub fn is_empty(&self) -> bool {
        let storage = self.store.storage.read().unwrap();
        let base = B::get_table(&storage.tables);
        base.data.is_empty()
    }

    /// Get a row of the table from the store by cloning the value.
    ///
    /// Returns `None` if there is no value for the specified key.
    pub fn get<Q>(&self, key: &Q) -> Option<B::Value>
    where
        B::Value: Clone,
        D::Key: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let storage = self.store.storage.read().unwrap();
        let base = B::get_table(&storage.tables);
        let index = D::get_table(&storage.tables);
        let base_key = index.data.get(key)?;
        base.data.get(base_key).cloned()
    }

    /// Get immutable access to a row of the table in the store by reference.
    ///
    /// Returns `None` (and does not call `f`) if there is no value for the specified key.
    pub fn with<Q, T>(&self, key: &Q, f: impl FnOnce(&B::Value) -> T) -> Option<T>
    where
        D::Key: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let storage = self.store.storage.read().unwrap();
        let base = B::get_table(&storage.tables);
        let index = D::get_table(&storage.tables);
        let base_key = index.data.get(key)?;
        let value = base.data.get(base_key)?;

        Some(f(value))
    }

    /// Insert a value into the table.
    ///
    /// Returns the previous value if there is one, or `None` if there is no value for the specified key.
    pub fn insert(&self, key: B::Key, value: B::Value) -> Option<B::Value>
    where
        B::Key: Clone,
    {
        let mut storage = self.store.storage.write().unwrap();
        let base = B::get_table_mut(&mut storage.tables);
        base.assert_or_set_owner(self.owner);
        base.indexes.on_insert(&key, &value);
        base.data.insert(key, value)
    }

    /// Get mutable access to a row of the table in the store in the store.
    ///
    /// Returns `None` (and does not call `f`) if there is no value for the specified key.
    pub fn mutate<Q, T>(&self, key: &Q, f: impl FnOnce(&mut B::Value) -> T) -> Option<T>
    where
        D::Key: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
        B::Key: Clone,
    {
        let mut storage = self.store.storage.write().unwrap();
        let index = D::get_table(&storage.tables);
        let base_key: *const <B as TableDesc>::Key = index.data.get(key)? as *const _;
        let base = B::get_table_mut(&mut storage.tables);
        // SAFETY: TODO
        let value = base.data.get_mut(unsafe { base_key.as_ref_unchecked() })?;
        // TODO we could be more efficient and only update indexes if the foreign key changes
        base.indexes.on_remove(value);
        let result = f(value);
        base.indexes
            .on_insert(unsafe { base_key.as_ref_unchecked() }, value);

        Some(result)
    }

    /// Remove a row from the table.
    ///
    /// Returns the previous value if there is one, or `None` if there is no value for the specified key.
    pub fn remove<Q>(&self, key: &Q) -> Option<B::Value>
    where
        D::Key: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let mut storage = self.store.storage.write().unwrap();
        let index = D::get_table_mut(&mut storage.tables);
        let base_key = index.data.remove(key)?;
        let base = B::get_table_mut(&mut storage.tables);
        let value = base.data.remove(base_key.borrow())?;
        base.indexes.on_remove(&value);
        Some(value)
    }
}

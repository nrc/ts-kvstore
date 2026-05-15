use crate::{Error, KvStore, Owner, Result, iter::TableIterator, schema, storage::SinValue};
use std::{borrow::Borrow, sync::Arc};

impl<TableStorage: schema::GeneratedStorage> KvStore<TableStorage> {
    // Non-transaction singleton API.
    // TODO Do we need try_ variations for all of these where we use try_read/try_write?

    pub fn get<D: schema::Singleton>(&self, _owner: Owner) -> Option<D::Value>
    where
        D::Value: Clone,
    {
        let storage = self.storage.read().unwrap();
        storage
            .get_singleton_value(&D::KEY)
            .map(|(_, v)| D::from_value_ref(v).clone())
    }

    pub fn get_arc<D: schema::ArcSingleton>(&self, _owner: Owner) -> Option<Arc<D::Value>> {
        let storage = self.storage.read().unwrap();
        storage
            .get_singleton_value(&D::KEY)
            .map(|(_, v)| D::from_value_arc(v))
    }

    pub fn with_value<D: schema::Singleton, T>(
        &self,
        _owner: Owner,
        f: impl FnOnce(&D::Value) -> T,
    ) -> Option<T> {
        let storage = self.storage.read().unwrap();
        let value = &storage.get_singleton_value(&D::KEY)?.1;
        let value = D::from_value_ref(value);
        Some(f(value))
    }

    // Question: do we need separate insert/update/upsert methods?
    pub fn insert<D: schema::Singleton>(
        &self,
        owner: Owner,
        value: D::ArgValue,
    ) -> Option<D::ArgValue> {
        let mut storage = self.storage.write().unwrap();
        let key = storage.hash(&D::KEY);
        storage
            .singletons
            .insert(key, (owner, D::to_value(value)))
            .map(|(_, v)| D::from_value(v))
    }

    pub fn mutate<D: schema::MutSingleton, T>(
        &self,
        _owner: Owner,
        mut f: impl FnMut(&mut D::Value) -> T,
    ) -> Option<T> {
        let mut storage = self.storage.write().unwrap();
        let value = storage.get_singleton_value_mut(&D::KEY)?.1;
        let value = D::from_value_mut(value);
        Some(f(value))
    }

    pub fn remove<D: schema::Singleton>(&self, _owner: Owner) -> Option<D::ArgValue> {
        let mut storage = self.storage.write().unwrap();
        let key = storage.hash(&D::KEY);
        storage
            .singletons
            .remove(&key)
            .map(|(_, v)| D::from_value(v))
    }

    // can be used to set owner without a value
    pub fn clear<D: schema::Singleton>(&self, owner: Owner) -> Option<D::ArgValue> {
        let mut storage = self.storage.write().unwrap();
        let key = storage.hash(&D::KEY);
        storage
            .singletons
            .insert(key, (owner, SinValue::None))
            .map(|(_, v)| D::from_value(v))
    }

    // Non-transaction table API.

    pub fn init_table<D: schema::TableDesc<TableStorage>>(&self, owner: Owner) -> Result<()> {
        let mut storage = self.storage.write().unwrap();
        let table = D::get_table_mut(&mut storage.tables);
        match &table.owner {
            Some(owner) => Err(Error::AlreadyInit(owner)),
            None => {
                table.owner = Some(owner);
                Ok(())
            }
        }
    }

    pub fn clear_table<D: schema::TableDesc<TableStorage>>(&self, _owner: Owner) {
        let mut storage = self.storage.write().unwrap();
        let table = D::get_table_mut(&mut storage.tables);
        table.data.clear();
    }

    pub fn iter_table<'a, D: schema::TableDesc<TableStorage> + 'static>(
        &'a self,
        _owner: Owner,
    ) -> impl Iterator<Item = (&'a D::Key, &'a D::Value)>
    where
        TableStorage: 'static,
    {
        let guard = self.storage.read().unwrap();
        TableIterator::<'a, TableStorage, D>::new(guard)
    }

    pub fn size_of_table<D: schema::TableDesc<TableStorage>>(&self) -> usize {
        let storage = self.storage.read().unwrap();
        let table = D::get_table(&storage.tables);
        table.data.len()
    }

    pub fn get_row<D: schema::TableDesc<TableStorage>>(
        &self,
        _owner: Owner,
        key: impl Borrow<D::Key>,
    ) -> Result<D::Value>
    where
        D::Value: Clone,
    {
        let storage = self.storage.read().unwrap();
        let table = D::get_table(&storage.tables);
        table
            .data
            .get(key.borrow())
            .cloned()
            .ok_or(Error::NotPresent)
    }

    pub fn with_row<D: schema::TableDesc<TableStorage>, T>(
        &self,
        _owner: Owner,
        f: impl FnOnce(&D::Value) -> T,
        key: impl Borrow<D::Key>,
    ) -> Option<T> {
        let storage = self.storage.read().unwrap();
        let table = D::get_table(&storage.tables);
        let value = table.data.get(key.borrow())?;

        Some(f(value))
    }

    pub fn insert_row<D: schema::TableDesc<TableStorage>>(
        &self,
        _owner: Owner,
        key: D::Key,
        value: D::Value,
    ) -> Option<D::Value> {
        let mut storage = self.storage.write().unwrap();
        let table = D::get_table_mut(&mut storage.tables);
        table.data.insert(key, value)
    }

    pub fn mutate_row<D: schema::TableDesc<TableStorage>, T>(
        &self,
        _owner: Owner,
        key: impl Borrow<D::Key>,
        mut f: impl FnMut(&mut D::Value) -> T,
    ) -> Option<T> {
        let mut storage = self.storage.write().unwrap();
        let table = D::get_table_mut(&mut storage.tables);
        let value = table.data.get_mut(key.borrow())?;

        Some(f(value))
    }

    pub fn take_row<D: schema::TableDesc<TableStorage>>(
        &self,
        _owner: Owner,
        key: impl Borrow<D::Key>,
    ) -> Option<D::Value> {
        let mut storage = self.storage.write().unwrap();
        let table = D::get_table_mut(&mut storage.tables);
        table.data.remove(key.borrow())
    }
}

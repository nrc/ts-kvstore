//! KvStore non-transactional API.

use crate::{Error, KvStore, Owner, Result, iter::TableIterator, schema, storage::SinValue};
use std::{borrow::Borrow, hash::Hash, marker::PhantomData, sync::Arc};

impl<TableStorage: schema::GeneratedStorage> KvStore<TableStorage> {
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

    pub fn with<D: schema::Singleton, T>(
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

    pub fn table<'a, D: schema::TableDesc<TableStorage>>(
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

pub struct KvTable<'a, TableStorage: schema::GeneratedStorage, D: schema::TableDesc<TableStorage>> {
    store: &'a KvStore<TableStorage>,
    owner: Owner,
    table: PhantomData<D>,
}

impl<'a, TableStorage: schema::GeneratedStorage, D: schema::TableDesc<TableStorage>>
    KvTable<'a, TableStorage, D>
{
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

    pub fn clear(&self) {
        let mut storage = self.store.storage.write().unwrap();
        let table = D::get_table_mut(&mut storage.tables);
        table.data.clear();
    }

    pub fn iter(&'a self) -> impl Iterator<Item = (&'a D::Key, &'a D::Value)>
    where
        TableStorage: 'static,
        D: 'static,
    {
        let guard = self.store.storage.read().unwrap();
        TableIterator::<'a, TableStorage, D>::new(guard)
    }

    pub fn len(&self) -> usize {
        let storage = self.store.storage.read().unwrap();
        let table = D::get_table(&storage.tables);
        table.data.len()
    }

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

    pub fn insert(&self, key: D::Key, value: D::Value) -> Option<D::Value> {
        let mut storage = self.store.storage.write().unwrap();
        let table = D::get_table_mut(&mut storage.tables);
        table.data.insert(key, value)
    }

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

    pub fn take<Q>(&self, key: &Q) -> Option<D::Value>
    where
        D::Key: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let mut storage = self.store.storage.write().unwrap();
        let table = D::get_table_mut(&mut storage.tables);
        table.data.remove(key.borrow())
    }
}

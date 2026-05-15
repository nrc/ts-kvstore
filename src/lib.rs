//! # An in-memory KV store for the Rust Tailscale client.

use std::{
    any::Any,
    borrow::Borrow,
    collections::HashMap,
    hash::{BuildHasher, RandomState},
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

mod hasher;
mod schema;

#[allow(private_bounds)]
pub struct KvStore<TableStorage: schema::GeneratedStorage> {
    storage: RwLock<Storage<TableStorage>>,
}

pub type Owner = &'static str;

#[allow(private_bounds)]
impl<TableStorage: schema::GeneratedStorage> KvStore<TableStorage> {
    // Non-transaction singleton API.
    // TODO Do we need try_ variations for all of these where we use try_read/try_write?

    pub fn get<D: schema::Singleton>(&self, _owner: Owner) -> Result<D::Value>
    where
        D::Value: Clone,
    {
        let storage = self.storage.read().unwrap();

        let key = storage.singleton_hash_builder.hash_one(D::KEY);
        storage
            .singletons
            .get(&key)
            .map(|(_, v)| D::from_value_ref(v).clone())
            .ok_or(Error::NotPresent)
    }

    pub fn get_arc<D: schema::Singleton>(&self, _owner: Owner) -> Result<Arc<D::Value>> {
        todo!()
    }

    pub fn with_value<D: schema::Singleton, T>(
        &self,
        _owner: Owner,
        f: impl FnOnce(&D::Value) -> T,
    ) -> Result<T> {
        let storage = self.storage.read().unwrap();

        let key = storage.singleton_hash_builder.hash_one(D::KEY);
        let value = &storage.singletons.get(&key).ok_or(Error::NotPresent)?.1;
        let value = D::from_value_ref(value);
        Ok(f(value))
    }

    pub fn insert<D: schema::Singleton>(
        &self,
        owner: Owner,
        value: D::ArgValue,
    ) -> Option<D::ArgValue> {
        let mut storage = self.storage.write().unwrap();

        let key = storage.singleton_hash_builder.hash_one(D::KEY);
        storage
            .singletons
            .insert(key, (owner, D::to_value(value)))
            .map(|(_, v)| D::from_value(v))
    }

    pub fn update<D: schema::Singleton>(&self, _owner: Owner, value: D::Value) -> Result<()> {
        Ok(())
    }
    pub fn mutate<D: schema::Singleton, T>(
        &self,
        _owner: Owner,
        f: impl FnMut(&mut D::Value) -> T,
    ) -> Result<T> {
        todo!()
    }
    pub fn upsert<D: schema::Singleton>(&self, _owner: Owner, value: D::Value) -> Result<()> {
        Ok(())
    }
    pub fn take<D: schema::Singleton>(&self, _owner: Owner) -> Option<D::Value> {
        None
    }
    // can be used to set owner without a value
    pub fn clear<D: schema::Singleton>(&self, _owner: Owner) -> Option<D::Value> {
        None
    }

    // Non-transaction table API.

    pub fn init_table<D: schema::TableDesc<TableStorage>>(&self, _owner: Owner) -> Result<()> {
        Err(Error::AlreadyInit)
    }

    pub fn clear_table<D: schema::TableDesc<TableStorage>>(&self, _owner: Owner) {}

    pub fn iter_table<D: schema::TableDesc<TableStorage>, T>(
        &self,
        _owner: Owner,
        f: impl Fn(&D::Value) -> T,
    ) -> T {
        todo!()
    }

    pub fn size_of_table<D: schema::TableDesc<TableStorage>>(&self) -> usize {
        todo!()
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
            .map(Clone::clone)
            .ok_or(Error::NotPresent)
    }

    pub fn with_row<D: schema::TableDesc<TableStorage>, T>(
        &self,
        _owner: Owner,
        f: impl FnOnce(&D::Value) -> T,
        key: &D::Key,
    ) -> Result<T> {
        let storage = self.storage.read().unwrap();

        todo!()
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

    pub fn update_row<D: schema::TableDesc<TableStorage>>(
        &self,
        _owner: Owner,
        key: &D::Key,
        value: D::Value,
    ) {
    }
    pub fn mutate_row<D: schema::TableDesc<TableStorage>, T>(
        &self,
        _owner: Owner,
        key: &D::Key,
        f: impl FnMut(&mut D::Value) -> T,
    ) -> Result<T> {
        todo!()
    }
    pub fn upsert_row<D: schema::TableDesc<TableStorage>>(
        &self,
        _owner: Owner,
        key: &D::Key,
        value: D::Value,
    ) {
    }
    pub fn take_row<D: schema::TableDesc<TableStorage>>(
        &self,
        _owner: Owner,
        key: &D::Key,
    ) -> Option<D::Value> {
        None
    }

    // Transactions.

    pub fn begin_transaction(&self, owner: Owner) -> Transaction<'_, TableStorage> {
        Transaction {
            guard: self.storage.write().unwrap(),
            owner,
        }
    }

    pub fn try_begin_transaction(&self, _owner: Owner) -> Option<Transaction<'_, TableStorage>> {
        None
    }

    pub fn begin_ro_transaction(&self, owner: Owner) -> RoTransaction<'_, TableStorage> {
        RoTransaction {
            guard: self.storage.read().unwrap(),
            owner,
        }
    }

    pub fn try_begin_ro_transaction(
        &self,
        _owner: Owner,
    ) -> Option<RoTransaction<'_, TableStorage>> {
        None
    }
}

#[allow(private_bounds)]
pub struct Transaction<'a, TableStorage: schema::GeneratedStorage> {
    guard: RwLockWriteGuard<'a, Storage<TableStorage>>,
    owner: Owner,
}

#[allow(private_bounds)]
impl<TableStorage: schema::GeneratedStorage> Transaction<'_, TableStorage> {
    // singleton and table API
}

#[allow(private_bounds)]
pub struct RoTransaction<'a, TableStorage: schema::GeneratedStorage> {
    guard: RwLockReadGuard<'a, Storage<TableStorage>>,
    owner: Owner,
}

#[allow(private_bounds)]
impl<TableStorage: schema::GeneratedStorage> RoTransaction<'_, TableStorage> {
    // singleton and table read API (get/with/iter)
}

#[derive(Debug, Clone)]
pub enum Error {
    AlreadyInit,
    AlreadyPresent,
    NotPresent,
    NotAnArc,
}

pub type Result<T> = std::result::Result<T, Error>;

struct Storage<TableStorage: schema::GeneratedStorage> {
    singletons: HashMap<u64, (Owner, SinValue), hasher::NoopU64Builder>,
    singleton_hash_builder: RandomState,
    tables: TableStorage,
}

impl<TableStorage: schema::GeneratedStorage> Storage<TableStorage> {
    pub fn new() -> Self {
        Storage {
            singletons: HashMap::with_hasher(hasher::NoopU64Builder),
            singleton_hash_builder: RandomState::new(),
            tables: TableStorage::default(),
        }
    }
}

#[derive(Default)]
enum SinValue {
    #[default]
    None,
    // TODO add other special cases
    U64(u64),
    Box(Box<dyn Any + Send + Sync>),
    Arc(Arc<dyn Any + Send + Sync>),
    Ref(&'static (dyn Any + Send + Sync)),
}

#[derive(Default)]
struct Table<D: schema::DataDesc> {
    owner: Option<Owner>,
    data: HashMap<D::Key, D::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello() {}
}

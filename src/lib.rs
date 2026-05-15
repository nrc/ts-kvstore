//! # An in-memory KV store for the Rust Tailscale client.

use std::{
    any::Any,
    borrow::Borrow,
    collections::HashMap,
    hash::{BuildHasher, RandomState},
    marker::PhantomPinned,
    pin::Pin,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockError},
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

    // Transactions.

    pub fn begin_transaction(&self, owner: Owner) -> Transaction<'_, TableStorage> {
        Transaction {
            guard: self.storage.write().unwrap(),
            owner,
        }
    }

    pub fn try_begin_transaction(&self, owner: Owner) -> Option<Transaction<'_, TableStorage>> {
        let guard = match self.storage.try_write() {
            Ok(g) => g,
            Err(TryLockError::WouldBlock) => return None,
            Err(TryLockError::Poisoned(_)) => panic!(),
        };
        Some(Transaction { guard, owner })
    }

    pub fn begin_ro_transaction(&self, owner: Owner) -> RoTransaction<'_, TableStorage> {
        RoTransaction {
            guard: self.storage.read().unwrap(),
            owner,
        }
    }

    pub fn try_begin_ro_transaction(
        &self,
        owner: Owner,
    ) -> Option<RoTransaction<'_, TableStorage>> {
        let guard = match self.storage.try_read() {
            Ok(g) => g,
            Err(TryLockError::WouldBlock) => return None,
            Err(TryLockError::Poisoned(_)) => panic!(),
        };
        Some(RoTransaction { guard, owner })
    }
}

#[track_caller]
fn assert_owner(_owner: Owner) {
    todo!()
}

#[allow(private_bounds)]
pub struct TableIterator<
    'a,
    TableStorage: schema::GeneratedStorage + 'static,
    D: schema::TableDesc<TableStorage> + 'static,
> {
    guard: RwLockReadGuard<'a, Storage<TableStorage>>,
    inner: Option<std::collections::hash_map::Iter<'static, D::Key, D::Value>>,
    _pin: PhantomPinned,
}

#[allow(private_bounds)]
impl<
    'a,
    TableStorage: schema::GeneratedStorage + 'static,
    D: schema::TableDesc<TableStorage> + 'static,
> TableIterator<'a, TableStorage, D>
{
    fn new(guard: RwLockReadGuard<'a, Storage<TableStorage>>) -> Pin<Box<Self>> {
        let mut result = Box::new(TableIterator {
            guard,
            inner: None,
            _pin: PhantomPinned,
        });
        let tables: *const _ = &result.guard.tables;
        // SAFETY: TODO
        let tables = unsafe { tables.as_ref_unchecked() };
        result.inner = Some(D::get_table(tables).data.iter());
        Box::into_pin(result)
    }

    fn project_inner(
        self: Pin<&mut Self>,
    ) -> Pin<&mut std::collections::hash_map::Iter<'static, D::Key, D::Value>> {
        unsafe { self.map_unchecked_mut(|this| this.inner.as_mut().unwrap()) }
    }
}

impl<'a, TableStorage: schema::GeneratedStorage, D: schema::TableDesc<TableStorage>> Iterator
    for Pin<Box<TableIterator<'a, TableStorage, D>>>
{
    type Item = (&'a D::Key, &'a D::Value);

    fn next(&mut self) -> Option<Self::Item> {
        self.as_mut().project_inner().next()
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
    AlreadyInit(Owner),
    NotPresent,
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

    fn hash(&self, thing: impl std::hash::Hash) -> u64 {
        self.singleton_hash_builder.hash_one(thing)
    }

    fn get_singleton_value(&self, key: impl std::hash::Hash) -> Option<&(Owner, SinValue)> {
        self.singletons.get(&self.hash(key))
    }

    fn get_singleton_value_mut(
        &mut self,
        key: impl std::hash::Hash,
    ) -> Option<(&Owner, &mut SinValue)> {
        self.singletons
            .get_mut(&self.hash(key))
            .map(|&mut (ref o, ref mut v)| (o, v))
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

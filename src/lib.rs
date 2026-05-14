//! # An in-memory KV store for the Rust Tailscale client.

use std::{
    any::Any,
    collections::HashMap,
    hash::BuildHasher,
    sync::{Arc, RwLock},
};

mod schema;

pub struct KvStore {
    storage: RwLock<Storage>,
}

pub type Owner = &'static str;

impl KvStore {
    pub fn new() -> Self {
        KvStore {
            storage: RwLock::new(Storage::new()),
        }
    }

    // Non-transaction singleton API.

    pub fn get<D: schema::Singleton>(&self, _owner: Owner) -> Result<D::Value>
    where
        D::Value: Clone,
    {
        let storage = self.storage.read().unwrap();

        let key = storage.singletons.hasher().hash_one(D::KEY);
        storage
            .singletons
            .get(&key)
            .map(|(_, v)| D::from_value(v).clone())
            .ok_or(Error::NotPresent)
    }

    pub fn with_value<D: schema::Singleton, T>(
        &self,
        _owner: Owner,
        f: impl FnOnce(&D::Value) -> T,
    ) -> Result<T> {
        let storage = self.storage.read().unwrap();

        let key = storage.singletons.hasher().hash_one(D::KEY);
        let value = &storage.singletons.get(&key).ok_or(Error::NotPresent)?.1;
        let value = D::from_value(value);
        Ok(f(value))
    }

    pub fn insert<D: schema::Singleton>(&self, owner: Owner, value: D::Value) -> Result<()> {
        Ok(())
    }
    pub fn update<D: schema::Singleton>(&self, owner: Owner, value: D::Value) -> Result<()> {
        Ok(())
    }
    pub fn mutate<D: schema::Singleton, T>(
        &self,
        owner: Owner,
        f: impl FnMut(&mut D::Value) -> T,
    ) -> Result<T> {
        todo!()
    }
    pub fn upsert<D: schema::Singleton>(&self, owner: Owner, value: D::Value) -> Result<()> {
        Ok(())
    }
    pub fn delete<D: schema::Singleton>(&self, owner: Owner) -> Result<()> {
        Ok(())
    }
    pub fn clear_value<D: schema::Singleton>(&self, owner: Owner) {}

    // Non-transaction table API.

    pub fn init_table<D: schema::TableDesc>(&self, owner: Owner) -> Result<()> {
        Err(Error::AlreadyInit)
    }

    pub fn clear_table<D: schema::TableDesc>(&self, owner: Owner) {}

    pub fn get_row<D: schema::TableDesc>(&self, _owner: Owner) -> Option<D::Value>
    where
        D::Value: Clone,
    {
        let storage = self.storage.read().unwrap();

        None
    }

    pub fn with_row<D: schema::TableDesc, T>(
        &self,
        _owner: Owner,
        f: impl FnOnce(&D::Value) -> T,
    ) -> Option<T> {
        let storage = self.storage.read().unwrap();

        None
    }

    pub fn iter_table<D: schema::TableDesc, T>(
        &self,
        _owner: Owner,
        f: impl Fn(&D::Value) -> T,
    ) -> T {
        todo!()
    }

    pub fn insert_row<D: schema::TableDesc>(&self, owner: Owner) {}
    pub fn update_row<D: schema::TableDesc>(&self, owner: Owner) {}
    pub fn mutate_row<D: schema::TableDesc, T>(
        &self,
        owner: Owner,
        f: impl FnMut(&mut D::Value) -> T,
    ) -> Result<T> {
        todo!()
    }
    pub fn upsert_row<D: schema::TableDesc>(&self, owner: Owner) {}
    pub fn delete_row<D: schema::TableDesc>(&self, owner: Owner) {}
    pub fn clear_row<D: schema::TableDesc>(&self, owner: Owner) {}

    // Transactions.

    pub fn begin_transaction(&self, _owner: Owner) -> Transaction {
        Transaction {}
    }

    pub fn try_begin_transaction(&self, _owner: Owner) -> Option<Transaction> {
        None
    }

    pub fn begin_ro_transaction(&self, _owner: Owner) -> RoTransaction {
        RoTransaction {}
    }

    pub fn try_begin_ro_transaction(&self, _owner: Owner) -> Option<RoTransaction> {
        None
    }
}

pub struct Transaction {}
pub struct RoTransaction {}

pub enum Error {
    AlreadyInit,
    AlreadyPresent,
    NotPresent,
}

pub type Result<T> = std::result::Result<T, Error>;

struct Storage {
    singletons: HashMap<u64, (Owner, SinValue)>,
}

impl Storage {
    pub fn new() -> Self {
        Storage {
            singletons: HashMap::new(),
        }
    }
}

enum SinValue {
    U64(u64),
    Box(Box<dyn Any + Send + Sync>),
    Arc(Arc<dyn Any + Send + Sync>),
    Ref(&'static (dyn Any + Send + Sync)),
}

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

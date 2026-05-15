use crate::{KvStore, Owner, schema, storage::Storage};
use std::sync::{RwLockReadGuard, RwLockWriteGuard, TryLockError};

impl<TableStorage: schema::GeneratedStorage> KvStore<TableStorage> {
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

pub struct Transaction<'a, TableStorage: schema::GeneratedStorage> {
    guard: RwLockWriteGuard<'a, Storage<TableStorage>>,
    owner: Owner,
}

impl<TableStorage: schema::GeneratedStorage> Transaction<'_, TableStorage> {
    // singleton and table API
}

pub struct RoTransaction<'a, TableStorage: schema::GeneratedStorage> {
    guard: RwLockReadGuard<'a, Storage<TableStorage>>,
    owner: Owner,
}

impl<TableStorage: schema::GeneratedStorage> RoTransaction<'_, TableStorage> {
    // singleton and table read API (get/with/iter)
}

//! KvStore transactional API.

use crate::{KvStore, Owner, schema, storage::Storage};
use std::sync::{RwLockReadGuard, RwLockWriteGuard, TryLockError};

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
            owner,
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
        Some(RoTransaction { guard, owner })
    }
}

/// TODO
pub struct Transaction<'a, TableStorage: schema::GeneratedStorage> {
    guard: RwLockWriteGuard<'a, Storage<TableStorage>>,
    owner: Owner,
}

impl<TableStorage: schema::GeneratedStorage> Transaction<'_, TableStorage> {
    // singleton and table API
}

/// TODO
pub struct RoTransaction<'a, TableStorage: schema::GeneratedStorage> {
    guard: RwLockReadGuard<'a, Storage<TableStorage>>,
    owner: Owner,
}

impl<TableStorage: schema::GeneratedStorage> RoTransaction<'_, TableStorage> {
    // singleton and table read API (get/with/iter)
}

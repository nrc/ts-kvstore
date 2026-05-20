//! Iterate over a table

use crate::{
    schema::{self, IndexDesc, TableDesc},
    storage::Storage,
};
use std::{
    marker::{PhantomData, PhantomPinned},
    ops::Deref,
    pin::Pin,
};

/// An iterator for a single table (described by the generic parameter `D`) in the KV store.
///
/// This is basically just a wrapper for an iterator over the `HashMap` representing the table.
/// However, we must hold a guard for the `KvStore`'s storage for the lifetime of the iterator and
/// that requires some tomfoolery with pinning.
///
/// A self-referential struct, so must be pinned once constructed. `TableIterator` is structurally
/// pinned, i.e., fields are pin-projected.
#[allow(private_bounds)]
pub struct TableIterator<
    'a,
    Guard: Deref<Target = Storage<TableStorage>> + 'a,
    TableStorage: schema::GeneratedStorage + 'static,
    D: TableDesc<Storage = TableStorage> + 'static,
> {
    /// Guard on the KV store's storage (all of it).
    guard: Guard,
    /// An iterator over the `HashMap` representing the table.
    ///
    /// Invariants:
    ///   - `inner.is_some()` one `new` has completed/`TableIterator` is pinned.
    ///   - The real lifetime of the iterator is 'a, rather than 'static. `self.guard` must keep the
    ///     read lock for the underlying storage for at least as long as `self.inner` exists.
    inner: Option<std::collections::hash_map::Iter<'static, D::Key, D::Value>>,
    /// `TableIterator` is `Unpin` because it requires `guard` to have a fixed address.
    _pin: PhantomPinned,
    _a: PhantomData<&'a D::Value>,
}

impl<
    'a,
    Guard: Deref<Target = Storage<TableStorage>> + 'a,
    TableStorage: schema::GeneratedStorage + 'static,
    D: TableDesc<Storage = TableStorage> + 'static,
> TableIterator<'a, Guard, TableStorage, D>
{
    /// Create an iterator over the table described by `D`.
    pub(crate) fn new(guard: Guard) -> Pin<Box<Self>> {
        let mut result = Box::new(TableIterator {
            guard,
            inner: None,
            _pin: PhantomPinned,
            _a: PhantomData,
        });
        let tables: *const _ = &result.guard.tables;
        // SAFETY: here we're extending the lifetime of the reference to the KV storage to `'static`.
        // We can't use a raw pointer because we won't be able to use that as input to create an
        // iterator. To ensure safety we must ensure that `self.guard` outlives `self.inner`.
        let tables = unsafe { tables.as_ref_unchecked() };
        result.inner = Some(D::get_table(tables).data.iter());
        Box::into_pin(result)
    }

    /// Accessor method for `self.inner`.
    fn project_inner(
        self: Pin<&mut Self>,
    ) -> Pin<&mut std::collections::hash_map::Iter<'static, D::Key, D::Value>> {
        // SAFETY: mapping the `Pin` to `self.inner` is safe because `TableIterator` is structually
        // pinned and we don't provide un-pinned access to any fields.
        // The unwrap is correct because once pinned, we guarantee that `self.inner.is_some()`.
        unsafe { self.map_unchecked_mut(|this| this.inner.as_mut().unwrap()) }
    }
}

impl<
    'a,
    Guard: Deref<Target = Storage<TableStorage>> + 'a,
    TableStorage: schema::GeneratedStorage,
    D: TableDesc<Storage = TableStorage>,
> Iterator for Pin<Box<TableIterator<'a, Guard, TableStorage, D>>>
{
    type Item = (&'a D::Key, &'a D::Value);

    fn next(&mut self) -> Option<Self::Item> {
        // Iterate by delegating to the `HashMap` iterator.
        self.as_mut().project_inner().next()
    }
}

impl<
    'a,
    Guard: Deref<Target = Storage<TableStorage>> + 'a,
    TableStorage: schema::GeneratedStorage,
    D: TableDesc<Storage = TableStorage>,
> Drop for TableIterator<'a, Guard, TableStorage, D>
{
    fn drop(&mut self) {
        // Ensure that `self.inner` is dropped before `self.guard`.
        self.inner = None;
    }
}

pub struct IndexIterator<
    'a,
    Guard: Deref<Target = Storage<TableStorage>> + 'a,
    TableStorage: schema::GeneratedStorage + 'static,
    D: IndexDesc<Storage = TableStorage, BaseTable = B> + 'static,
    B: TableDesc<Storage = TableStorage>,
> {
    guard: Guard,
    /// An iterator over the `HashMap` representing the table.
    ///
    /// Invariants:
    ///   - `inner.is_some()` one `new` has completed/`TableIterator` is pinned.
    ///   - The real lifetime of the iterator is 'a, rather than 'static. `self.guard` must keep the
    ///     read lock for the underlying storage for at least as long as `self.inner` exists.
    inner: Option<std::collections::hash_map::Iter<'static, D::Key, D::Value>>,
    /// `TableIterator` is `Unpin` because it requires `guard` to have a fixed address.
    _pin: PhantomPinned,
    _a: PhantomData<&'a B::Value>,
}

impl<
    'a,
    Guard: Deref<Target = Storage<TableStorage>> + 'a,
    TableStorage: schema::GeneratedStorage + 'static,
    D: IndexDesc<Storage = TableStorage, BaseTable = B> + 'static,
    B: TableDesc<Storage = TableStorage>,
> IndexIterator<'a, Guard, TableStorage, D, B>
{
    /// Create an iterator over the table described by `D`.
    pub(crate) fn new(guard: Guard) -> Pin<Box<Self>> {
        let mut result = Box::new(IndexIterator {
            guard,
            inner: None,
            _pin: PhantomPinned,
            _a: PhantomData,
        });
        let tables: *const _ = &result.guard.tables;
        // SAFETY: here we're extending the lifetime of the reference to the KV storage to `'static`.
        // We can't use a raw pointer because we won't be able to use that as input to create an
        // iterator. To ensure safety we must ensure that `self.guard` outlives `self.inner`.
        let tables = unsafe { tables.as_ref_unchecked() };
        result.inner = Some(D::get_table(tables).data.iter());
        Box::into_pin(result)
    }

    /// Accessor method for `self.inner`.
    fn project_inner(
        self: Pin<&mut Self>,
    ) -> Pin<&mut std::collections::hash_map::Iter<'static, D::Key, D::Value>> {
        // SAFETY: mapping the `Pin` to `self.inner` is safe because `TableIterator` is structually
        // pinned and we don't provide un-pinned access to any fields.
        // The unwrap is correct because once pinned, we guarantee that `self.inner.is_some()`.
        unsafe { self.map_unchecked_mut(|this| this.inner.as_mut().unwrap()) }
    }
}

impl<
    'a,
    Guard: Deref<Target = Storage<TableStorage>> + 'a,
    TableStorage: schema::GeneratedStorage + 'static,
    D: IndexDesc<Storage = TableStorage, BaseTable = B, Value = B::Key> + 'static,
    B: TableDesc<Storage = TableStorage> + 'static,
> Iterator for Pin<Box<IndexIterator<'a, Guard, TableStorage, D, B>>>
{
    type Item = (&'a D::Key, &'a B::Value);

    fn next(&mut self) -> Option<Self::Item> {
        // Iterate by delegating to the `HashMap` iterator.
        self.as_mut().project_inner().next().and_then(|(k, bk)| {
            let tables: *const _ = &self.guard.tables;
            // SAFETY: TODO
            let tables = unsafe { tables.as_ref_unchecked() };
            let base = B::get_table(tables);
            Some((k, base.data.get(bk)?))
        })
    }
}

impl<
    'a,
    Guard: Deref<Target = Storage<TableStorage>> + 'a,
    TableStorage: schema::GeneratedStorage + 'static,
    D: IndexDesc<Storage = TableStorage, BaseTable = B> + 'static,
    B: TableDesc<Storage = TableStorage>,
> Drop for IndexIterator<'a, Guard, TableStorage, D, B>
{
    fn drop(&mut self) {
        // Ensure that `self.inner` is dropped before `self.guard`.
        self.inner = None;
    }
}

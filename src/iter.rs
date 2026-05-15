use crate::{schema, storage::Storage};
use std::{marker::PhantomPinned, pin::Pin, sync::RwLockReadGuard};

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
    pub(crate) fn new(guard: RwLockReadGuard<'a, Storage<TableStorage>>) -> Pin<Box<Self>> {
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

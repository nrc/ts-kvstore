use crate::{Owner, schema};
use std::{
    any::Any,
    collections::HashMap,
    hash::{BuildHasher, RandomState},
    sync::Arc,
};

pub(crate) struct Storage<TableStorage: schema::GeneratedStorage> {
    pub(crate) singletons: HashMap<u64, (Owner, SinValue), crate::hasher::NoopU64Builder>,
    singleton_hash_builder: RandomState,
    pub(crate) tables: TableStorage,
}

impl<TableStorage: schema::GeneratedStorage> Storage<TableStorage> {
    pub(crate) fn new() -> Self {
        Storage {
            singletons: HashMap::with_hasher(crate::hasher::NoopU64Builder),
            singleton_hash_builder: RandomState::new(),
            tables: TableStorage::default(),
        }
    }

    pub(crate) fn hash(&self, thing: impl std::hash::Hash) -> u64 {
        self.singleton_hash_builder.hash_one(thing)
    }

    pub(crate) fn get_singleton_value(
        &self,
        key: impl std::hash::Hash,
    ) -> Option<&(Owner, SinValue)> {
        self.singletons.get(&self.hash(key))
    }

    pub(crate) fn get_singleton_value_mut(
        &mut self,
        key: impl std::hash::Hash,
    ) -> Option<(&Owner, &mut SinValue)> {
        self.singletons
            .get_mut(&self.hash(key))
            .map(|&mut (ref o, ref mut v)| (o, v))
    }
}

#[derive(Default)]
pub(crate) enum SinValue {
    #[default]
    None,
    // TODO add other special cases
    U64(u64),
    Box(Box<dyn Any + Send + Sync>),
    Arc(Arc<dyn Any + Send + Sync>),
    Ref(&'static (dyn Any + Send + Sync)),
}

#[derive(Default)]
pub(crate) struct Table<D: schema::DataDesc> {
    pub(crate) owner: Option<Owner>,
    pub(crate) data: HashMap<D::Key, D::Value>,
}

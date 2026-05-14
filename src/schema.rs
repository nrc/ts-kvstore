use std::{any::Any, hash::Hash};

pub trait DataDesc: Sized {
    type Key: Hash;
    type Value: Any + Send + Sync;
}

pub trait Singleton: DataDesc {
    const KEY: Self::Key;

    fn from_value(value: &crate::SinValue) -> &Self::Value;
    fn to_value(value: Self::Value) -> crate::SinValue;
}

pub trait TableDesc: DataDesc {
    const NAME: &'static str;

    fn get_table(storage: &crate::Storage) -> &crate::Table<Self>;
    fn get_table_mut(storage: &mut crate::Storage) -> &mut crate::Table<Self>;
}

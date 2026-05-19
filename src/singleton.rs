//! Helpers for working with singleton KVs.

use crate::{Owner, storage::SinValue};

/// Helper trait to handle `SinValue::None`
pub trait OptSingletonValue {
    type Value;
    fn map_singleton_value<T>(self, f: impl FnOnce(Self::Value) -> T) -> Option<T>;
}

impl<'a> OptSingletonValue for Option<&'a (Owner, SinValue)> {
    type Value = &'a SinValue;
    fn map_singleton_value<T>(self, f: impl FnOnce(Self::Value) -> T) -> Option<T> {
        match self? {
            (_, SinValue::None) => None,
            (_, v) => Some(f(v)),
        }
    }
}

impl<'a> OptSingletonValue for Option<(&'a Owner, &'a mut SinValue)> {
    type Value = &'a mut SinValue;
    fn map_singleton_value<T>(self, f: impl FnOnce(Self::Value) -> T) -> Option<T> {
        match self? {
            (_, SinValue::None) => None,
            (_, v) => Some(f(v)),
        }
    }
}

impl OptSingletonValue for Option<(Owner, SinValue)> {
    type Value = SinValue;
    fn map_singleton_value<T>(self, f: impl FnOnce(SinValue) -> T) -> Option<T> {
        match self? {
            (_, SinValue::None) => None,
            (_, v) => Some(f(v)),
        }
    }
}

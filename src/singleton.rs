//! Helpers for working with singleton KVs.

use crate::{
    Owner,
    storage::{SinValue, Storage},
};

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

#[track_caller]
pub fn assert_owner(
    owner: Owner,
    key: u64,
    storage: &Storage<impl crate::schema::GeneratedStorage>,
) {
    #[cfg(debug_assertions)]
    if let Some(prev_owner) = storage.get_singleton_owner(key) {
        assert_eq!(
            prev_owner, owner,
            "Ownership violation: expected {prev_owner}, found {owner}"
        );
    }
}

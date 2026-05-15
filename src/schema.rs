#![allow(private_interfaces, private_bounds, unused_macros)]

use crate::storage::{SinValue, Table};
use std::{any::Any, hash::Hash, sync::Arc};

pub trait DataDesc: Sized {
    type Key: Hash + Eq;
    type Value: Any + Send + Sync;
}

pub trait Singleton: DataDesc {
    const KEY: Self::Key;
    type ArgValue;

    fn from_value(value: SinValue) -> Self::ArgValue;
    fn from_value_ref(value: &SinValue) -> &Self::Value;
    fn to_value(value: Self::ArgValue) -> SinValue;
}

pub trait ArcSingleton: Singleton {
    fn from_value_arc(value: &SinValue) -> Arc<Self::Value> {
        match value {
            SinValue::Arc(a) => a.clone().downcast().unwrap(),
            _ => unreachable!(),
        }
    }
}

pub trait MutSingleton: Singleton {
    fn from_value_mut(value: &mut SinValue) -> &mut Self::Value;
}

pub trait TableDesc<Storage: GeneratedStorage>: DataDesc {
    const NAME: &'static str;

    fn get_table(storage: &Storage) -> &Table<Self>;
    fn get_table_mut(storage: &mut Storage) -> &mut Table<Self>;
}

#[doc(hidden)]
pub trait GeneratedStorage: Default {}

#[macro_export]
macro_rules! singleton {
    ($name: ident ($key: expr, $key_ty: ty, u64)) => {
        singleton!($name($key, $key_ty, u64, u64, U64));

        impl $crate::schema::MutSingleton for $name {
            fn from_value_mut(value: &mut $crate::storage::SinValue) -> &mut Self::Value {
                match value {
                    $crate::match_helper_lhs!(U64, v) => $crate::match_helper_rhs_mut!(U64, v),
                    _ => unreachable!(),
                }
            }
        }
    };
    ($name: ident ($key: expr, $key_ty: ty, $value_ty: ty, Box)) => {
        singleton!($name($key, $key_ty, $value_ty, $value_ty, Box));

        impl $crate::schema::MutSingleton for $name {
            fn from_value_mut(value: &mut $crate::storage::SinValue) -> &mut Self::Value {
                match value {
                    $crate::match_helper_lhs!(Box, v) => $crate::match_helper_rhs_mut!(Box, v),
                    _ => unreachable!(),
                }
            }
        }
    };
    ($name: ident ($key: expr, $key_ty: ty, $value_ty: ty, Arc)) => {
        singleton!($name($key, $key_ty, $value_ty, std::sync::Arc<$value_ty>, Arc));

        impl $crate::schema::ArcSingleton for $name {}
    };
    ($name: ident ($key: expr, $key_ty: ty, $value_ty: ty, Ref)) => {
        singleton!($name($key, $key_ty, $value_ty, &'static $value_ty, Ref))
    };
    ($name: ident ($key: expr, $key_ty: ty, $value_ty: ty, $arg_value_ty: ty, $variant: ident)) => {
        pub struct $name;

        impl $crate::schema::DataDesc for $name {
            type Key = $key_ty;
            type Value = $value_ty;
        }

        impl $crate::schema::Singleton for $name {
            const KEY: $key_ty = $key;
            type ArgValue = $arg_value_ty;

            fn from_value(value: $crate::storage::SinValue) -> Self::ArgValue {
                match value {
                    $crate::match_helper_lhs!($variant, v) => {
                        $crate::match_helper_rhs!($variant, v)
                    }
                    _ => unreachable!(),
                }
            }

            fn from_value_ref(value: &$crate::storage::SinValue) -> &Self::Value {
                match value {
                    $crate::match_helper_lhs!($variant, v) => {
                        $crate::match_helper_rhs_ref!($variant, v)
                    }
                    _ => unreachable!(),
                }
            }

            fn to_value(value: Self::ArgValue) -> $crate::storage::SinValue {
                $crate::init_helper!($variant, value)
            }
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! init_helper {
    (U64, $value: ident) => {
        $crate::storage::SinValue::U64($value)
    };
    (Box, $value: ident) => {
        $crate::storage::SinValue::Box(Box::new($value) as Box<dyn Any + Send + Sync>)
    };
    (Arc, $value: ident) => {
        $crate::storage::SinValue::Arc($value.clone() as Arc<dyn Any + Send + Sync>)
    };
    (Ref, $value: ident) => {
        $crate::storage::SinValue::Ref($value as &'static (dyn Any + Send + Sync))
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! match_helper_lhs {
    (U64, $value: ident) => {
        $crate::storage::SinValue::U64($value)
    };
    ($variant: ident, $value: ident) => {
        $crate::storage::SinValue::$variant($value)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! match_helper_rhs {
    (U64, $value: ident) => {
        $value
    };
    (Box, $value: ident) => {
        *$value.downcast().unwrap()
    };
    (Arc, $value: ident) => {
        $value.downcast().unwrap()
    };
    (Ref, $value: ident) => {
        $value.downcast_ref().unwrap()
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! match_helper_rhs_ref {
    (U64, $value: ident) => {
        $value
    };
    ($variant: ident, $value: ident) => {
        $value.downcast_ref().unwrap()
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! match_helper_rhs_mut {
    (U64, $value: ident) => {
        $value
    };
    ($variant: ident, $value: ident) => {
        $value.downcast_mut().unwrap()
    };
}

#[macro_export]
macro_rules! tables {
    ($($name: ident ($key_ty: ty, $value_ty: ty)),*) => {
        $(
            #[derive(Default)]
            pub struct $name;

            impl $crate::schema::DataDesc for $name {
                type Key = $key_ty;
                type Value = $value_ty;
            }

            impl $crate::schema::TableDesc<TableStorage> for $name {
                const NAME: &'static str = stringify!($name);

                fn get_table(storage: &TableStorage) -> &$crate::storage::Table<Self> {
                    &storage.$name
                }
                fn get_table_mut(storage: &mut TableStorage) -> &mut $crate::storage::Table<Self> {
                    &mut storage.$name
                }
            }
        )*

        #[derive(Default)]
        #[allow(non_snake_case)]
        pub struct TableStorage {
            $($name: $crate::storage::Table<$name>),*
        }
        impl $crate::schema::GeneratedStorage for TableStorage {}

        pub struct KvStore($crate::KvStore<TableStorage>);

        impl KvStore {
            pub fn new() -> Self {
                KvStore($crate::KvStore::new_with_storage(std::sync::RwLock::new($crate::storage::Storage::new())))
            }
        }

        impl std::ops::Deref for KvStore {
            type Target = $crate::KvStore<TableStorage>;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };
}

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn single() {
        singleton!(Foo("hello", &'static str, u64, Box));
        singleton!(Bar("hello", &'static str, u64, Arc));
        singleton!(Baz("hello", &'static str, u64, Ref));
        singleton!(Qux("hello", &'static str, u64));

        assert_eq!(&42, Foo::from_value_ref(&Foo::to_value(42)));
        assert_eq!(&42, Bar::from_value_ref(&Bar::to_value(Arc::new(42))));
        assert_eq!(&42, Baz::from_value_ref(&Baz::to_value(&42)));
        assert_eq!(&42, Qux::from_value_ref(&Qux::to_value(42)));

        tables!();

        let store = KvStore::new();
        store.insert::<Foo>("owner", 42);
        assert_eq!(store.get::<Foo>("owner").unwrap(), 42);
    }

    #[test]
    fn table() {
        tables!(Foo(&'static str, String), Bar(u32, Vec<String>));

        let store = KvStore::new();

        store.insert_row::<Foo>("owner", "hello", "world".to_owned());
        assert_eq!(store.get_row::<Foo>("owner", "hello").unwrap(), "world");

        store.insert_row::<Bar>("owner", 5, vec!["boo".to_owned(), "bang".to_owned()]);
        assert_eq!(
            store.get_row::<Bar>("owner", 5).unwrap(),
            vec!["boo".to_owned(), "bang".to_owned()]
        );
    }
}

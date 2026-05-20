//! Traits and macros for defining the KvStore schema.
#![allow(private_interfaces, private_bounds, unused_macros)]

use crate::storage::{SinValue, Table};
use std::{any::Any, hash::Hash, sync::Arc};

// TODO it would be nice if we could just use the type of the value as the key, but that has
// problems with the orphan rule.
/// A singleton key/value.
///
/// Prefer to use the macros in this module rather than this trait directly.
pub trait Singleton: 'static {
    /// The type of the value.
    type Value: Any + Send + Sync;
    /// The type used to initialize and access the value.
    type ArgValue;

    fn from_value(value: SinValue) -> Self::ArgValue;
    fn from_value_ref(value: &SinValue) -> &Self::Value;
    fn to_value(value: Self::ArgValue) -> SinValue;
}

/// A singleton key/value which is store as an `Arc`.
///
/// Implementing this trait for non-`Arc` values will cause [`crate::KvStore::get_arc`] to panic (but is
/// not unsafe).
///
/// Prefer to use the macros in this module rather than this trait directly.
pub trait ArcSingleton: Singleton {
    fn from_value_arc(value: &SinValue) -> Arc<Self::Value> {
        match value {
            SinValue::Arc(a) => a.clone().downcast().unwrap(),
            _ => unreachable!(),
        }
    }
}

/// Mark a singleton value as mutable (i.e., the value in the store is unique).
///
/// Prefer to use the macros in this module rather than this trait directly.
pub trait MutSingleton: Singleton {
    fn from_value_mut(value: &mut SinValue) -> &mut Self::Value;
}

/// Describes tabular key/values in the store.
///
/// Prefer to use the macros in this module rather than this trait directly.
pub trait TableDesc: Sized {
    /// The type of the key.
    type Key: Hash + Eq;
    /// The type of the value.
    type Value: Any + Send + Sync;
    /// The name of the table.
    const NAME: &'static str;
    /// The storage for the table.
    type Storage: GeneratedStorage;

    fn get_table(storage: &Self::Storage) -> &Table<Self>;
    fn get_table_mut(storage: &mut Self::Storage) -> &mut Table<Self>;
}

/// Marker trait to indicate a storage implementation.
///
/// This should be considered a sealed trait and not implemented except by the macros in this module.
/// Unfortunately it has to be public because of macro visibility hygiene.
#[doc(hidden)]
pub trait GeneratedStorage: Default {}

/// Macro to declare a singleton key/value in the store.
///
/// Does not need to be used within or near the store declaration, but also is not linked to a specific
/// store. Using a generated accessor on a store different to the store the key/value was stored in
/// will have unpredictable results (panics, memory safety, etc.).
///
/// # Syntax:
///
/// - `singleton!(u64)` to declare a value with type `u64` and inline storage.
/// - `singleton!(ValueType, Box)` to declare a value with type `Box<ValueType>`.
/// - `singleton!(ValueType, Arc)` to declare a value with type `Arc<ValueType>`.
/// - `singleton!(ValueType, Ref)` to declare a value with type `&'static ValueType`.
///
/// The storage class is separate to the value type since they have different representations in the
/// store and slightly different APIs (e.g., whether mutable access is supported or access by cloning
/// or copying a shared reference).
#[macro_export]
macro_rules! singleton {
    ($name: ident (u64)) => {
        singleton!($name(u64, u64, U64));

        impl $crate::schema::MutSingleton for $name {
            fn from_value_mut(value: &mut $crate::storage::SinValue) -> &mut Self::Value {
                match value {
                    $crate::match_helper_lhs!(U64, v) => $crate::match_helper_rhs_mut!(U64, v),
                    _ => unreachable!(),
                }
            }
        }
    };
    ($name: ident ($value_ty: ty, Box)) => {
        singleton!($name($value_ty, $value_ty, Box));

        impl $crate::schema::MutSingleton for $name {
            fn from_value_mut(value: &mut $crate::storage::SinValue) -> &mut Self::Value {
                match value {
                    $crate::match_helper_lhs!(Box, v) => $crate::match_helper_rhs_mut!(Box, v),
                    _ => unreachable!(),
                }
            }
        }
    };
    ($name: ident ($value_ty: ty, Arc)) => {
        singleton!($name($value_ty, std::sync::Arc<$value_ty>, Arc));

        impl $crate::schema::ArcSingleton for $name {}
    };
    ($name: ident ($value_ty: ty, Ref)) => {
        singleton!($name($value_ty, &'static $value_ty, Ref));
    };
    ($name: ident ($value_ty: ty, $arg_value_ty: ty, $variant: ident)) => {
        /// Describes a singleton in the KV store.
        #[allow(non_camel_case_types)]
        pub struct $name;

        impl $crate::schema::Singleton for $name {
            type Value = $value_ty;
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

/// Declare the tables in a key/value store. Generates the store itself with the specified tables.
///
/// The syntax is `tables!(Name(KeyType, ValueType),*)`, where `Name` is an identifier to name
/// the table, and `KeyType` and `ValueType` are types. `Name` is used as a type argument to
/// KvStore methods to identify the table. Use with an empty list of tables to generate a store
/// for use only with singleton key/value pairs.
///
/// # Example:
///
/// ```rust
/// # use ts_kvstore::tables;
/// # pub struct Node;
/// # pub trait Edge {}
/// tables!(
///   Nodes(&'static str, Node),
///   Edges(u32, Box<dyn Edge + Send + Sync>)
/// );
/// ```
#[macro_export]
macro_rules! tables {
    ($($name: ident ($key_ty: ty, $value_ty: ty)),*) => {
        $(
            /// Describes a table in the KV store.
            #[derive(Default)]
            pub struct $name;

            impl $crate::schema::TableDesc for $name {
                type Key = $key_ty;
                type Value = $value_ty;
                const NAME: &'static str = stringify!($name);
                type Storage = TableStorage;

                fn get_table(storage: &TableStorage) -> &$crate::storage::Table<Self> {
                    &storage.$name
                }
                fn get_table_mut(storage: &mut TableStorage) -> &mut $crate::storage::Table<Self> {
                    &mut storage.$name
                }
            }
        )*

        /// Macro-generated storage for all tabular data.
        #[derive(Default)]
        #[allow(non_snake_case)]
        pub struct TableStorage {
            $($name: $crate::storage::Table<$name>),*
        }
        impl $crate::schema::GeneratedStorage for TableStorage {}

        /// A key-value store.
        ///
        /// See [`$crate::KvStore`] (which this type implicitly derefences to) for full docs.
        pub struct KvStore($crate::KvStore<TableStorage>);

        impl KvStore {
            /// Create a new, empty KV store as described by the schema macros.
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
        singleton!(Foo(u64, Box));
        singleton!(Bar(u64, Arc));
        singleton!(Baz(u64, Ref));
        singleton!(Qux(u64));

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

        store
            .table::<Foo>("owner")
            .insert("hello", "world".to_owned());
        assert_eq!(store.table::<Foo>("owner").get("hello").unwrap(), "world");

        store
            .table::<Bar>("owner")
            .insert(5, vec!["boo".to_owned(), "bang".to_owned()]);
        assert_eq!(
            store.table::<Bar>("owner").get(&5).unwrap(),
            vec!["boo".to_owned(), "bang".to_owned()]
        );
    }
}

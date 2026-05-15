#![allow(private_interfaces, private_bounds, unused_macros)]

use std::{any::Any, hash::Hash};

pub trait DataDesc: Sized {
    type Key: Hash + Eq;
    type Value: Any + Send + Sync;
}

pub trait Singleton: DataDesc {
    const KEY: Self::Key;
    type ArgValue;

    fn from_value(value: crate::SinValue) -> Self::ArgValue;
    fn from_value_ref(value: &crate::SinValue) -> &Self::Value;
    fn to_value(value: Self::ArgValue) -> crate::SinValue;
}

pub trait TableDesc<Storage: GeneratedStorage>: DataDesc {
    const NAME: &'static str;

    fn get_table(storage: &Storage) -> &crate::Table<Self>;
    fn get_table_mut(storage: &mut Storage) -> &mut crate::Table<Self>;
}

pub(crate) trait GeneratedStorage: Default {}

// TODO how to use these externally?
#[macro_export]
macro_rules! singleton {
    ($name: ident ($key: expr, $key_ty: ty, u64)) => {
        singleton!($name($key, $key_ty, u64, u64, U64))
    };
    ($name: ident ($key: expr, $key_ty: ty, $value_ty: ty, Box)) => {
        singleton!($name($key, $key_ty, $value_ty, $value_ty, Box))
    };
    // TODO we really want to take &Arc, but can't make it an assoc type
    ($name: ident ($key: expr, $key_ty: ty, $value_ty: ty, Arc)) => {
        singleton!($name($key, $key_ty, $value_ty, std::sync::Arc<$value_ty>, Arc))
    };
    ($name: ident ($key: expr, $key_ty: ty, $value_ty: ty, Ref)) => {
        singleton!($name($key, $key_ty, $value_ty, &'static $value_ty, Ref))
    };
    ($name: ident ($key: expr, $key_ty: ty, $value_ty: ty, $arg_value_ty: ty, $variant: ident)) => {
        pub struct $name;

        impl DataDesc for $name {
            type Key = $key_ty;
            type Value = $value_ty;
        }

        impl Singleton for $name {
            const KEY: $key_ty = $key;
            type ArgValue = $arg_value_ty;

            fn from_value(value: crate::SinValue) -> Self::ArgValue {
                match value {
                    match_helper_lhs!($variant, v) => match_helper_rhs!($variant, v),
                    _ => unreachable!(),
                }
            }

            fn from_value_ref(value: &crate::SinValue) -> &Self::Value {
                match value {
                    match_helper_lhs!($variant, v) => match_helper_rhs_ref!($variant, v),
                    _ => unreachable!(),
                }
            }

            fn to_value(value: Self::ArgValue) -> crate::SinValue {
                init_helper!($variant, value)
            }
        }
    };
}

macro_rules! init_helper {
    (U64, $value: ident) => {
        crate::SinValue::U64($value)
    };
    (Box, $value: ident) => {
        crate::SinValue::Box(Box::new($value) as Box<dyn Any + Send + Sync>)
    };
    (Arc, $value: ident) => {
        crate::SinValue::Arc($value.clone() as Arc<dyn Any + Send + Sync>)
    };
    (Ref, $value: ident) => {
        crate::SinValue::Ref($value as &'static (dyn Any + Send + Sync))
    };
}

macro_rules! match_helper_lhs {
    (U64, $value: ident) => {
        crate::SinValue::U64($value)
    };
    ($variant: ident, $value: ident) => {
        crate::SinValue::$variant($value)
    };
}

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

macro_rules! match_helper_rhs_ref {
    (U64, $value: ident) => {
        $value
    };
    ($variant: ident, $value: ident) => {
        $value.downcast_ref().unwrap()
    };
}

#[macro_export]
macro_rules! tables {
    ($($name: ident ($key_ty: ty, $value_ty: ty)),*) => {
        // allow case mismatch

        $(
            #[derive(Default)]
            pub struct $name;

            impl DataDesc for $name {
                type Key = $key_ty;
                type Value = $value_ty;
            }

            impl TableDesc<TableStorage> for $name {
                const NAME: &'static str = stringify!($name);

                fn get_table(storage: &TableStorage) -> &crate::Table<Self> {
                    &storage.$name
                }
                fn get_table_mut(storage: &mut TableStorage) -> &mut crate::Table<Self> {
                    &mut storage.$name
                }
            }
        )*

        #[derive(Default)]
        #[allow(non_snake_case)]
        struct TableStorage {
            $($name: $crate::Table<$name>),*
        }
        impl crate::schema::GeneratedStorage for TableStorage {}

        pub type KvStore = $crate::KvStore<TableStorage>;

        impl KvStore {
            pub fn new() -> Self {
                crate::KvStore {
                    storage: std::sync::RwLock::new(crate::Storage::new()),
                }
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

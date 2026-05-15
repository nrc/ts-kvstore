use ts_kvstore::{singleton, tables};

singleton!(Qux("hello", &'static str, u64));
tables!(Foo(&'static str, String), Bar(u32, Vec<String>));

pub fn main() {
    let store = KvStore::new();
}

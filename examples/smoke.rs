use ts_kvstore::{Owner, singleton, tables};

const OWNER: Owner = "owner";

singleton!(foo(u64));
singleton!(bar(u64));

tables!(Nodes(u32 => String), Edges(String => (u32, u32)));

pub fn main() {
    let store = KvStore::new();

    store.insert::<foo>(OWNER, 42);
    store.insert::<bar>(OWNER, 0);

    let nodes = store.table::<Nodes>(OWNER);
    nodes.insert(4, "a".to_owned());
    nodes.insert(0, "b".to_owned());
    nodes.insert(10, "c".to_owned());
    nodes.insert(400, "d".to_owned());

    let edges = store.table::<Edges>(OWNER);
    edges.insert("X".to_owned(), (0, 4));
    edges.insert("Y".to_owned(), (10, 4));
    edges.insert("Z".to_owned(), (400, 400));

    edges.mutate("Z", |(a, b)| {
        *a += 1;
        *b += 1;
    });

    assert_eq!(nodes.len(), 4);

    edges.iter().for_each(|(k, (a, b))| {
        let a_name = match nodes.get(a) {
            Some(s) => format!(" ({s})"),
            None => String::new(),
        };
        let b_name = match nodes.get(b) {
            Some(s) => format!(" ({s})"),
            None => String::new(),
        };
        println!("{k}: {a}{a_name}, {b}{b_name}");
    });
}

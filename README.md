# An in-memory KV store for the Rust Tailscale client.

We're using ident-based keys for the singleton KVs, but we might want to actually look them up
by key. Should we have an `impl Hash` API as well as or instead of the idents?

For ident-keys, do we even need keys and key types? Should remove them and just hash the self type
if we keep typed singletons.

TODO

- refactor
  - use Arc rather than Pin and unsafe for iterator? (only works with Tokio's RwLock)
  - general re-org stuff
- secondary indexes
 
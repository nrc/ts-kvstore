# An in-memory KV store for the Rust Tailscale client.

We're using ident-based keys for the singleton KVs, but we might want to actually look them up
by key. Should we have an `impl Hash` API as well as or instead of the idents? How would we do
typed values?

For ident-keys, do we even need keys and key types?

TODO

- docs
  - crate-level
- async?
  - replace RwLock with Tokio version
  - do we want to be async?
    - if we want to `await` inside transactions
    - can we afford to block the whole thread when waiting for the lock?
- check-in on static/dynamic-ness of singleton values
- tests
- refactor
  - use Arc rather than Pin and unsafe for iterator? (only works with Tokio's RwLock)
- check ownership
  - handle SinValue::None
- transactional API
 
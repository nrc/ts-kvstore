# An in-memory KV store for the Rust Tailscale client.

We're using ident-based keys for the singleton KVs, but we might want to actually look them up
by key. Should we have an `impl Hash` API as well as or instead of the idents? How would we do
typed values?

For ident-keys, do we even need keys and key types?

TODO

- docs
  - crate-level
- async!
- check-in on static/dynamic-ness of singleton values
- tests
- refactor
- check ownership
  - handle SinValue::None
- transactional API

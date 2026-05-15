# An in-memory KV store for the Rust Tailscale client.

We're using ident-based keys for the singleton KVs, but we might want to actually look them up
by key. Should we have an `impl Hash` API as well as or instead of the idents? How would we do
typed values?

For ident-keys, do we even need keys and key types?

TODO

- tests
- docs
  - safety
  - SinValue
  - table iterator
  - singletons hashmap and hasher
  - schema traits
  - macros
- refactor
- check ownership
- transactional API

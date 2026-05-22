# An in-memory KV store for the Rust Tailscale client.

TODO

- secondary indexes
  - update on for_each_mut
  - transactions
  - values and keys iterators
  - optionally with functions rather than field selection
  - should we panic on double insertions? We'd need to track in on_insert whether we expect an old value or not
  - do we need to support multi-indexes?
- pub/sub
  - subscribe/unsub
  - notifications
  - blocking get and subscribe
- refactor
  - could we factor out the locking mechanism from raw/transaction so the operations could be shared?
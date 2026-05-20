//! A no-op hasher to be used on already hashed data. Used to store singleton KVs which are hashed
//! externally to the HashMap.

use std::hash::{BuildHasher, Hasher};

/// A builder for creating [`NoopU64Hasher`].
pub struct NoopU64Builder;

impl BuildHasher for NoopU64Builder {
    type Hasher = NoopU64Hasher;

    fn build_hasher(&self) -> Self::Hasher {
        NoopU64Hasher { value: None }
    }
}

/// A hasher which passes through exactly one `u64` and panics on any other input.
pub struct NoopU64Hasher {
    value: Option<u64>,
}

impl Hasher for NoopU64Hasher {
    /// Panics if a `u64` has not been hashed.
    fn finish(&self) -> u64 {
        self.value.unwrap()
    }

    /// Always panics.
    fn write(&mut self, _bytes: &[u8]) {
        panic!();
    }

    /// Panics if a `u64` has already been hashed.
    fn write_u64(&mut self, i: u64) {
        match &self.value {
            None => self.value = Some(i),
            Some(_) => panic!(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::hash::Hash;

    #[test]
    fn hash_u64() {
        #![allow(clippy::manual_hash_one)]

        let mut hasher = NoopU64Builder.build_hasher();
        42u64.hash(&mut hasher);
        assert_eq!(42, hasher.finish());
    }

    #[test]
    #[should_panic]
    fn hash_not_u64() {
        let mut hasher = NoopU64Builder.build_hasher();
        "hello".hash(&mut hasher);
    }
}

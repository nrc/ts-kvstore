use std::hash::{BuildHasher, Hasher};

pub struct NoopU64Builder;

impl BuildHasher for NoopU64Builder {
    type Hasher = NoopU64Hasher;

    fn build_hasher(&self) -> Self::Hasher {
        NoopU64Hasher { value: None }
    }
}

pub struct NoopU64Hasher {
    value: Option<u64>,
}

impl Hasher for NoopU64Hasher {
    fn finish(&self) -> u64 {
        self.value.unwrap()
    }

    fn write(&mut self, _bytes: &[u8]) {
        panic!();
    }

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

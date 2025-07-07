//!
//! # Database Abstraction
//!

pub use crate::datatabase::{Database, DatabaseResource, Query};
pub use crate::db::Db;

mod datatabase {
    use ::ironx_core::Stable;

    pub trait DatabaseResource: Stable {}

    pub trait Database<T: DatabaseResource>: Stable {
        fn query<Q>(&self, query: &Q) -> impl Future<Output = Result<Q::Success, Q::Failure>>
        where
            Q: Query<T>;
    }

    pub trait Query<T: DatabaseResource>: Stable {
        type Success;
        type Failure;

        fn call(&self, resource: &T) -> impl Future<Output = Result<Self::Success, Self::Failure>>;
    }
}
mod db {
    use crate::{Database, DatabaseResource, Query};
    use ::ironx_core::Resource;
    use std::marker::PhantomData;

    #[derive(Debug, Clone)]
    pub struct Db<T: DatabaseResource, D: Resource<T>>(D, PhantomData<T>);

    impl<T: DatabaseResource, D: Resource<T>> Database<T> for Db<T, D> {
        async fn query<Q>(&self, query: &Q) -> Result<Q::Success, Q::Failure>
        where
            Q: Query<T>,
        {
            query.call(self.0.resource()).await
        }
    }

    impl<T: DatabaseResource, D: Resource<T>> Db<T, D> {
        pub fn new(resource: D) -> Self {
            Self(resource, PhantomData)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Debug, Clone)]
    struct Registry(HashMap<u8, u8>);

    impl Registry {
        fn new(key: u8, val: u8) -> Self {
            Self(HashMap::from_iter([(key, val)]))
        }
    }

    impl DatabaseResource for Registry {}

    #[derive(Debug, Clone)]
    struct FetchValue(u8);

    impl Query<Registry> for FetchValue {
        type Success = u8;
        type Failure = ();

        async fn call(&self, resource: &Registry) -> Result<Self::Success, Self::Failure> {
            resource.0.get(&self.0).copied().ok_or(())
        }
    }

    #[tokio::test]
    async fn it_works() {
        let db = Db::new(Registry::new(11, 42));
        let num = db.query(&FetchValue(11)).await.unwrap();
        assert_eq!(num, 42);
    }
}

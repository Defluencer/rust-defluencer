mod config;
mod deserialization;
mod iterators;
mod tree;

use std::iter;

use cid::Cid;

use futures::Stream;
use futures_util::StreamExt;

use ipfs_api::IpfsService;

use self::config::{Config, Tree};

use super::{
    errors::Error,
    traits::{Key, Value},
};

#[derive(Clone)]
pub struct MerkelSearchTree {
    config: Config,

    ipfs: IpfsService,

    root: Cid,
}

impl MerkelSearchTree {
    pub fn new(ipfs: IpfsService, config: Option<Config>) -> Result<Self, Error> {
        let root = Cid::default();

        let config = config.unwrap_or_default();

        let tree = Self { config, ipfs, root };

        Ok(tree)
    }

    pub async fn load(ipfs: IpfsService, cid: Cid) -> Result<Self, Error> {
        let tree = ipfs.dag_get::<&str, Tree>(cid, None).await?;

        let Tree { config, root } = tree;

        let config = ipfs.dag_get::<&str, Config>(config, None).await?;

        let tree = Self { ipfs, config, root };

        Ok(tree)
    }

    pub async fn get<K: Key, V: Value>(&self, key: K) -> Result<Option<V>, Error> {
        let mut results: Vec<Result<(K, V), Error>> =
            tree::batch_get::<K, V>(self.ipfs.clone(), self.root, iter::once(key))
                .await
                .collect()
                .await;

        match results.pop() {
            Some(result) => result.map(|(_, value)| Some(value)),
            None => return Ok(None),
        }
    }

    pub async fn batch_get<K: Key, V: Value>(
        &self,
        keys: impl IntoIterator<Item = K>,
    ) -> impl Stream<Item = Result<(K, V), Error>> {
        tree::batch_get::<K, V>(self.ipfs.clone(), self.root, keys).await
    }

    pub async fn insert<K: Key, V: Value>(&mut self, key: K, value: V) -> Result<(), Error> {
        let root = tree::batch_insert(
            self.ipfs.clone(),
            self.root,
            self.config.clone(),
            iter::once((key, value)),
        )
        .await?;

        self.root = root;

        Ok(())
    }

    pub async fn batch_insert<K: Key, V: Value>(
        &mut self,
        key_values: impl IntoIterator<Item = (K, V)>,
    ) -> Result<(), Error> {
        let root = tree::batch_insert(
            self.ipfs.clone(),
            self.root,
            self.config.clone(),
            key_values,
        )
        .await?;

        self.root = root;

        Ok(())
    }

    pub async fn remove<K: Key, V: Value>(&mut self, key: K) -> Result<(), Error> {
        let root = tree::batch_remove::<K, V>(
            self.ipfs.clone(),
            self.root,
            self.config.clone(),
            iter::once(key),
        )
        .await?;

        self.root = root;

        Ok(())
    }

    pub fn stream<K: Key, V: Value>(&self) -> impl Stream<Item = Result<(K, V), Error>> {
        tree::values(self.ipfs.clone(), self.root)
    }
}

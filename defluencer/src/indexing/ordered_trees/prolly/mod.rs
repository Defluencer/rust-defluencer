mod config;
mod deserialization;
mod iterators;
mod node;
mod tree;

use std::iter;

pub use config::{Config, HashThreshold, Strategies};

use cid::Cid;

use futures::{Stream, StreamExt};

use ipfs_api::IpfsService;

use config::Tree;

use super::{errors::Error, traits::Value};

type Key = Vec<u8>;

#[derive(Clone)]
pub struct ProllyTree {
    config: Config,

    ipfs: IpfsService,

    root: Cid,
}

impl ProllyTree {
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

    pub async fn get<V: Value>(&self, key: Key) -> Result<Option<(Key, V)>, Error> {
        let results = tree::batch_get(self.ipfs.clone(), self.root, iter::once(key))
            .collect::<Vec<_>>()
            .await;

        let results: Result<Vec<_>, _> = results.into_iter().collect();
        let mut results = results?;

        if results.len() != 1 {
            return Ok(None);
        }

        let kv = results.pop().unwrap();

        Ok(Some(kv))
    }

    pub fn batch_get<V: Value>(
        &self,
        keys: impl IntoIterator<Item = Key>,
    ) -> impl Stream<Item = Result<(Key, V), Error>> {
        tree::batch_get(self.ipfs.clone(), self.root, keys)
    }

    pub async fn insert<V: Value>(&mut self, key: Key, value: V) -> Result<(), Error> {
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

    pub async fn batch_insert<V: Value>(
        &mut self,
        key_values: impl IntoIterator<Item = (Key, V)>,
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

    pub async fn remove<V: Value>(&mut self, key: Key) -> Result<(), Error> {
        let root = tree::batch_remove::<Key, V>(
            self.ipfs.clone(),
            self.root,
            self.config.clone(),
            iter::once(key),
        )
        .await?;

        self.root = root;

        Ok(())
    }

    pub async fn batch_remove<V: Value>(
        &mut self,
        keys: impl IntoIterator<Item = Key>,
    ) -> Result<(), Error> {
        let root =
            tree::batch_remove::<Key, V>(self.ipfs.clone(), self.root, self.config.clone(), keys)
                .await?;

        self.root = root;

        Ok(())
    }

    pub fn stream<V: Value>(&self) -> impl Stream<Item = Result<(Key, V), Error>> {
        tree::stream_pairs(self.ipfs.clone(), self.root)
    }
}

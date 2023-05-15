mod config;
mod deserialization;
mod iterators;
mod node;
mod tree;

use std::iter;

use cid::Cid;

use futures::{Stream, StreamExt};

use ipfs_api::{responses::Codec, IpfsService};

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
        let tree = ipfs
            .dag_get::<&str, Tree>(cid, None, Codec::default())
            .await?;

        let Tree { config, root } = tree;

        let config = ipfs
            .dag_get::<&str, Config>(config, None, Codec::default())
            .await?;

        let tree = Self { ipfs, config, root };

        Ok(tree)
    }

    pub async fn save(&self) -> Result<Cid, Error> {
        let config = self
            .ipfs
            .dag_put(&self.config, self.config.codec, self.config.codec)
            .await?;

        let tree = Tree {
            config,
            root: self.root,
        };

        let cid = self
            .ipfs
            .dag_put(&tree, self.config.codec, self.config.codec)
            .await?;

        Ok(cid)
    }

    pub async fn get<K: Key, V: Value>(&self, key: K) -> Result<Option<(K, V)>, Error> {
        let results = tree::batch_get::<K, V>(
            self.ipfs.clone(),
            self.root,
            self.config.codec,
            iter::once(key),
        )
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

    pub fn batch_get<K: Key, V: Value>(
        &self,
        keys: impl IntoIterator<Item = K>,
    ) -> impl Stream<Item = Result<(K, V), Error>> {
        tree::batch_get::<K, V>(self.ipfs.clone(), self.root, self.config.codec, keys)
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

    pub async fn batch_remove<K: Key, V: Value>(
        &mut self,
        keys: impl IntoIterator<Item = K>,
    ) -> Result<(), Error> {
        let root =
            tree::batch_remove::<K, V>(self.ipfs.clone(), self.root, self.config.clone(), keys)
                .await?;

        self.root = root;

        Ok(())
    }

    pub fn stream<K: Key, V: Value>(&self) -> impl Stream<Item = Result<(K, V), Error>> {
        tree::stream_pairs(self.ipfs.clone(), self.root, self.config.codec)
    }
}

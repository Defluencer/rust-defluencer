mod config;
mod tree;

use std::iter;

use async_recursion::async_recursion;

use futures::{
    channel::mpsc::{self, Sender},
    future::join_all,
    stream, Stream, StreamExt, TryStreamExt,
};

use either::Either;

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::types::IPLDLink;

use crate::errors::Error;

use config::{Config, Strategies, Tree};

use multihash::Code;

//use self::tree::{batch_get, Key, TreeNode, Value};

#[derive(Clone)]
pub struct ProllyTree {
    hash_fn: Code,

    codec: Codec,

    strategy: Strategies,

    ipfs: IpfsService,

    root: IPLDLink,
}

impl ProllyTree {
    /* pub async fn new<K: Key, V: Value>(
        hash_fn: Code,
        codec: Codec,
        strategy: Strategies,
        ipfs: IpfsService,
    ) -> Result<Self, Error> {
        let node = TreeNode::<K, V>::default();
        let cid = ipfs.dag_put(&node, codec).await?;
        let root = cid.into();

        let tree = Self {
            hash_fn,
            codec,
            strategy,
            ipfs,
            root,
        };

        Ok(tree)
    } */

    /* pub async fn load(link: IPLDLink, ipfs: IpfsService) -> Result<Self, Error> {
        let tree = ipfs.dag_get::<&str, Tree>(link.into(), None).await?;

        let (config, root) = tree.into_inner();

        let config = ipfs.dag_get::<&str, Config>(config.into(), None).await?;

        let tree = Self {
            hash_fn: config.hash_fn(),
            codec: config.codec(),
            strategy: config.chunking_strat(),
            ipfs,
            root,
        };

        Ok(tree)
    } */

    /* pub async fn get<K: Key, V: Value>(&self, key: K) -> Option<V> {
        todo!()
    } */

    /* pub async fn batch_get<K: Key, V: Value>(
        &self,
        keys: impl Iterator<Item = K> + IntoIterator<Item = K>,
    ) -> Vec<V> {
        let stream = tree::batch_get(self.ipfs.clone(), self.root, keys);

        //TODO
    } */

    /* pub async fn insert<K: Key, V: Value>(&mut self, key: K, value: V) -> Result<(), Error> {
        let root = tree::batch_insert(
            self.ipfs.clone(),
            self.root,
            self.strategy,
            iter::once((key, value)),
        )
        .await?;

        self.root = root;

        Ok(())
    } */

    /* pub async fn batch_insert<K: Key, V: Value>(
        &mut self,
        key_values: impl Iterator<Item = (K, V)> + IntoIterator<Item = (K, V)>,
    ) -> Result<(), Error> {
        let root =
            tree::batch_insert(self.ipfs.clone(), self.root, self.strategy, key_values).await?;

        self.root = root;

        Ok(())
    } */

    /* pub async fn remove<K: Key, V: Value>(&self, key: K) -> Option<V> {
        todo!()
    } */

    /* pub async fn batch_remove<K: Key, V: Value>(
        &self,
        key_values: impl Iterator<Item = K> + IntoIterator<Item = K>,
    ) -> Vec<V> {
        todo!()
    } */

    /* pub async fn stream<K: Key, V: Value>(&self) -> impl Stream<Item = Result<(K, V), Error>> {
        tree::stream(self.ipfs.clone(), self.root)
    } */
}

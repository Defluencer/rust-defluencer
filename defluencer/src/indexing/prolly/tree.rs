use std::{
    fmt::{self, Debug},
    marker::PhantomData,
    num::NonZeroU32,
    ops::{Bound, RangeBounds},
};

use async_recursion::async_recursion;

use futures::{
    channel::mpsc::{self, Sender},
    future::{join_all, try_join_all},
    stream, Stream, StreamExt, TryStreamExt,
};

use either::Either;

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::types::IPLDLink;

use crate::errors::Error;

use serde::{
    de::{self, DeserializeOwned, MapAccess, SeqAccess, Visitor},
    ser::SerializeSeq,
    Deserialize, Deserializer, Serialize,
};

use multihash::{Hasher, Sha2_256};

use super::config::{ChunkingStrategy, Strategies, Tree};

use libipld_core::ipld::Ipld;

use cid::Cid;

const CHUNKING_FACTOR: u32 = 16;

pub trait Key:
    Default
    + Debug
    + Clone
    + Eq
    + Ord
    + Serialize
    + DeserializeOwned
    + From<Vec<u8>>
    + Into<Vec<u8>>
    + Send
    + Sync
    + Sized
    + AsRef<[u8]>
    + 'static
{
}
impl<
        T: Default
            + Debug
            + Clone
            + Eq
            + Ord
            + Serialize
            + DeserializeOwned
            + From<Vec<u8>>
            + Into<Vec<u8>>
            + Send
            + Sync
            + Sized
            + AsRef<[u8]>
            + 'static,
    > Key for T
{
}

pub trait Value:
    Default
    + Debug
    + Clone
    + Eq
    + Serialize
    + DeserializeOwned
    + From<Vec<u8>>
    + Send
    + Sync
    + Sized
    + 'static
{
}
impl<
        T: Default
            + Debug
            + Clone
            + Eq
            + Serialize
            + DeserializeOwned
            + From<Vec<u8>>
            + Send
            + Sync
            + Sized
            + 'static,
    > Value for T
{
}

/// Type state for tree leaf nodes
#[derive(Debug, Clone)]
struct Leaf<V> {
    elements: Vec<V>,
}

/// Type state for tree branch nodes
#[derive(Debug, Clone)]
struct Branch {
    links: Vec<Cid>,
}

pub trait TreeNodeType {}
impl<V> TreeNodeType for Leaf<V> {}
impl TreeNodeType for Branch {}

#[derive(Debug, Default, Clone)]
pub struct TreeNode<K, T: TreeNodeType> {
    keys: Vec<K>, //TODO represent keys as bytes so they can be ordered.
    values: T,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "Ipld")]
enum TreeNodes<K, V> {
    #[serde(bound = "K: Key")]
    Branch(TreeNode<K, Branch>),
    #[serde(bound = "V: Value")]
    Leaf(TreeNode<K, Leaf<V>>),
}

impl<K: Key, V: Value> TryFrom<Ipld> for TreeNodes<K, V> {
    type Error = Error;

    fn try_from(value: Ipld) -> Result<Self, Self::Error> {
        let mut list = match value {
            Ipld::List(list) => list,
            _ => return Err(Error::NotFound),
        };

        let values = match list.remove(2) {
            Ipld::List(values) => values,
            _ => return Err(Error::NotFound),
        };

        let keys = match list.remove(1) {
            Ipld::List(keys) => keys,
            _ => return Err(Error::NotFound),
        };

        let keys = keys
            .into_iter()
            .filter_map(|ipld| match ipld {
                Ipld::Bytes(key) => Some(key.into()),
                _ => None,
            })
            .collect();

        let is_leaf = match list.remove(0) {
            Ipld::Bool(is_leaf) => is_leaf,
            _ => return Err(Error::NotFound),
        };

        let tree = if is_leaf {
            let elements = values
                .into_iter()
                .filter_map(|ipld| match ipld {
                    Ipld::Bytes(value) => Some(value.into()),
                    _ => None,
                })
                .collect();

            let values = Leaf { elements };

            TreeNodes::Leaf(TreeNode { keys, values })
        } else {
            let links = values
                .into_iter()
                .filter_map(|ipld| match ipld {
                    Ipld::Link(value) => Some(value),
                    _ => None,
                })
                .collect();

            let values = Branch { links };

            TreeNodes::Branch(TreeNode { keys, values })
        };

        Ok(tree)
    }
}

impl<K: Key, V: Value> Serialize for TreeNodes<K, V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            TreeNodes::Branch(branch_node) => {
                let length = 1 + branch_node.keys.len() + branch_node.values.links.len();
                let mut seq = serializer.serialize_seq(Some(length))?;

                seq.serialize_element(&false)?;

                for key in branch_node.keys.iter() {
                    seq.serialize_element(key)?;
                }

                for link in branch_node.values.links.iter() {
                    let ipld: Ipld = link.into();
                    seq.serialize_element(&ipld)?;
                }

                seq.end()
            }
            TreeNodes::Leaf(leaf_node) => {
                let length = 1 + leaf_node.keys.len() + leaf_node.values.elements.len();
                let mut seq = serializer.serialize_seq(Some(length))?;

                seq.serialize_element(&true)?;

                for key in leaf_node.keys.iter() {
                    seq.serialize_element(key)?;
                }

                for element in leaf_node.values.elements.iter() {
                    seq.serialize_element(element)?;
                }

                seq.end()
            }
        }
    }
}

impl<K: Key, T: TreeNodeType> TreeNode<K, T> {
    /// Find the index for each key in the batch
    fn search<'a>(&'a self, batch: Vec<K>) -> impl Iterator<Item = usize> + 'a {
        batch
            .into_iter()
            .scan((self, 0usize), |(node, start), key| {
                match node.keys[*start..].binary_search(&key) {
                    Ok(idx) => {
                        *start = idx;
                        return Some(idx);
                    }
                    Err(_) => None,
                }
            })
    }
}

impl<K: Key> TreeNode<K, Branch> {
    /// Insert sorted keys and links into this node.
    ///
    /// Idempotent.
    fn insert(
        &mut self,
        key_values: impl IntoIterator<Item = (K, Cid)> + Iterator<Item = (K, Cid)>,
    ) {
        let mut start = 0;
        for (key, value) in key_values {
            match self.keys[start..].binary_search(&key) {
                Ok(idx) => {
                    self.keys[idx] = key;
                    self.values.links[idx] = value;
                    start = idx;
                }
                Err(idx) => {
                    self.keys.insert(idx, key);
                    self.values.links.insert(idx, value);
                    start = idx;
                }
            }
        }
    }

    /// Split the batch into smaller batch with associated node links.
    fn split_batch<'a, V: Value>(
        &'a self,
        batch: Vec<(K, V)>,
    ) -> impl Iterator<Item = (Vec<(K, V)>, Cid)> + 'a {
        //TODO refactor into one scan call
        batch
            .into_iter()
            .scan((self, 0usize), |(node, start), (key, value)| {
                match node.keys[*start..].binary_search(&key) {
                    Ok(idx) => {
                        let link = node.values.links[idx];

                        *start = idx;
                        return Some(((key, value), link));
                    }
                    Err(idx) => {
                        if idx == 0 {
                            return node.values.links.first().map(|&item| ((key, value), item));
                        }

                        if idx == node.keys.len() {
                            return node.values.links.last().map(|&item| ((key, value), item));
                        }

                        let link = node.values.links[idx - 1];

                        *start = idx;
                        return Some(((key, value), link));
                    }
                }
            })
            .scan(
                (Option::<Vec<(K, V)>>::None, Option::<Cid>::None),
                |(batch, batch_link), ((key, value), link)| {
                    if batch.is_none() || batch_link.is_none() {
                        *batch = Some(vec![(key, value)]);
                        *batch_link = Some(link);

                        return None;
                    }

                    if *batch_link.as_ref().unwrap() == link {
                        batch.as_mut().unwrap().push((key, value));

                        return None;
                    }

                    let batch = batch.take().unwrap();
                    let batch_link = batch_link.take().unwrap();

                    return Some((batch, batch_link));
                },
            )
    }

    /// Run the chunking algorithm on this node. Return splitted nodes.
    fn split_into(mut self, chunking: impl ChunkingStrategy) -> Vec<Self> {
        //TODO Find the boundary indexes then split the nodes, should be simpler.

        let mut key_count = self.keys.len();
        let mut value_count = self.values.links.len();

        let mut result = Vec::new();

        let mut node = Option::<TreeNode<K, Branch>>::None;
        for i in 0..self.keys.len() {
            let key = self.keys.remove(i);

            let is_boundary = chunking.boundary(key.as_ref());

            if is_boundary {
                if let Some(node) = node.take() {
                    key_count -= node.keys.len();
                    value_count -= node.values.links.len();

                    result.push(node);
                }

                let new_node = Self {
                    keys: Vec::with_capacity(key_count),
                    values: Branch {
                        links: Vec::with_capacity(value_count),
                    },
                };

                node = Some(new_node);
            }

            // Guaranteed node because first key is always a boundary
            node.as_mut().unwrap().keys.push(key);

            let value = self.values.links[i];
            node.as_mut().unwrap().values.links.push(value);

            if i == self.keys.len() - 1 {
                let node = node.take().unwrap();
                result.push(node);
            }
        }

        result
    }

    /// Merge all node keys and links with other
    fn merge(&mut self, other: Self) {
        self.insert(other.keys.into_iter().zip(other.values.links.into_iter()))
    }

    /// Remove key and links that match batch keys
    ///
    /// Idempotent.
    fn remove_batch(&mut self, batch: impl IntoIterator<Item = K> + Iterator<Item = K>) {
        let mut start = 0;
        for batch_key in batch {
            if let Ok(idx) = self.keys[start..].binary_search(&batch_key) {
                self.keys.remove(idx);
                self.values.links.remove(idx);

                start = idx;
            }
        }
    }

    pub fn iter(&self) -> BranchIterator<K> {
        BranchIterator {
            node: self,
            index: 0,
        }
    }

    /* pub fn into_iter(self) -> BranchIntoIterator<K> {
        BranchIntoIterator {
            node: self,
            index: 0,
        }
    } */
}

pub struct BranchIterator<'a, K: Key> {
    node: &'a TreeNode<K, Branch>,
    index: usize,
}

impl<'a, K: Key> Iterator for BranchIterator<'a, K> {
    type Item = ((Bound<&'a K>, Bound<&'a K>), &'a Cid);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.node.keys.len() {
            return None;
        }

        let key = &self.node.keys[self.index];
        let l_bound = Bound::Included(key);

        let h_bound = match self.node.keys.get(self.index + 1) {
            Some(key) => Bound::Excluded(key),
            None => Bound::Unbounded,
        };

        let range = (l_bound, h_bound);
        let link = &self.node.values.links[self.index];

        Some((range, link))
    }
}

/* pub struct BranchIntoIterator<K: Key> {
    node: TreeNode<K, Branch>,
    index: usize,
}

impl<K: Key> Iterator for BranchIntoIterator<K> {
    type Item = ((Bound<K>, Bound<K>), Cid);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.node.keys.len() {
            return None;
        }

        let key = self.node.keys[self.index];
        let l_bound = Bound::Included(key);

        let h_bound = match self.node.keys.get(self.index + 1) {
            Some(key) => Bound::Excluded(*key),
            None => Bound::Unbounded,
        };

        let range = (l_bound, h_bound);
        let link = self.node.values.links[self.index];

        Some((range, link))
    }
} */

impl<K: Key, V: Value> TreeNode<K, Leaf<V>> {
    /// Insert sorted keys and values into this node.
    ///
    /// Idempotent.
    fn insert(&mut self, key_values: impl IntoIterator<Item = (K, V)> + Iterator<Item = (K, V)>) {
        let mut start = 0;
        for (key, value) in key_values {
            match self.keys[start..].binary_search(&key) {
                Ok(idx) => {
                    self.keys[idx] = key;
                    self.values.elements[idx] = value;
                    start = idx;
                }
                Err(idx) => {
                    self.keys.insert(idx, key);
                    self.values.elements.insert(idx, value);
                    start = idx;
                }
            }
        }
    }

    /// Run the chunking algorithm on this node. Return splitted nodes.
    fn split_into(mut self, chunking: impl ChunkingStrategy) -> Vec<Self> {
        //TODO Find the boundary indexes then split the nodes, should be simpler.

        //TODO return an iterator

        let mut key_count = self.keys.len();
        let mut value_count = self.values.elements.len();

        let mut result = Vec::new();

        let mut node = Option::<TreeNode<K, Leaf<V>>>::None;
        for i in 0..self.keys.len() {
            let key = self.keys.remove(i);

            let is_boundary = chunking.boundary(key.as_ref());

            if is_boundary {
                if let Some(node) = node.take() {
                    key_count -= node.keys.len();
                    value_count -= node.values.elements.len();

                    result.push(node);
                }

                let new_node = Self {
                    keys: Vec::with_capacity(key_count),
                    values: Leaf {
                        elements: Vec::with_capacity(value_count),
                    },
                };

                node = Some(new_node);
            }

            // Guaranteed node because first key is always a boundary
            node.as_mut().unwrap().keys.push(key);

            let value = self.values.elements.remove(i);
            node.as_mut().unwrap().values.elements.push(value);

            if i == self.keys.len() - 1 {
                let node = node.take().unwrap();
                result.push(node);
            }
        }

        result
    }

    /// Merge all node elements with other
    fn merge(&mut self, other: Self) {
        self.insert(
            other
                .keys
                .into_iter()
                .zip(other.values.elements.into_iter()),
        )
    }

    /// Remove keys and values that match batch keys
    ///
    /// Idempotent.
    fn remove_batch(&mut self, batch: impl IntoIterator<Item = K> + Iterator<Item = K>) {
        let mut start = 0;
        for batch_key in batch {
            if let Ok(idx) = self.keys[start..].binary_search(&batch_key) {
                self.keys.remove(idx);
                self.values.elements.remove(idx);

                start = idx;
            }
        }
    }

    pub fn iter(&self) -> impl IntoIterator<Item = (&K, &V)> {
        self.keys.iter().zip(self.values.elements.iter())
    }

    pub fn into_iter(self) -> impl IntoIterator<Item = (K, V)> {
        self.keys.into_iter().zip(self.values.elements.into_iter())
    }
}

/* /// Return all KVs for the keys provided.
pub fn batch_get<K: Key, V: Value>(
    ipfs: IpfsService,
    root: IPLDLink,
    keys: impl Iterator<Item = K> + IntoIterator<Item = K>,
) -> impl Stream<Item = Result<(K, V), Error>> {
    stream::once({
        let ipfs = ipfs.clone();

        let mut batch = keys.into_iter().collect::<Vec<_>>();

        batch.sort_unstable();

        async move {
            ipfs.dag_get::<&str, TreeNode<K, V>>(root.into(), None)
                .await
                .map(|node| (node, batch))
        }
    })
    .map_ok(move |(node, batch)| stream_batch_get(ipfs.clone(), node, batch))
    .try_flatten()
} */

/* fn stream_batch_get<K: Key, V: Value>(
    ipfs: IpfsService,
    node: TreeNode<K, V>,
    mut batch: Vec<K>,
) -> impl Stream<Item = Result<(K, V), Error>> {
    let iter = if node.0 {
        let iter = node.search_leaf(batch).map(|item| Ok(item));

        stream::iter(iter)
    } else {
        //TODO  recursively call itself until a leaf is found

        let iter = node.search_branch(batch);
    }

    stream::try_unfold(, move |mut iter| {
            let ipfs = ipfs.clone();

            async move {
                let (batch, link) = match iter.next() {
                    Some(item) => item,
                    None => return Result::<_, Error>::Ok(None),
                };

                let node = ipfs
                    .dag_get::<&str, TreeNode<K, V>>(root.into(), None)
                    .await?;

                let stream = stream_batch_get(ipfs, node, batch).boxed_local();

                Some(Ok((stream, iter)))
            }
        })
        .try_flatten()
} */

/* /// Return all KVs for the keys provided.
pub async fn batch_get<K: Key, V: Value>(
    ipfs: IpfsService,
    tree: IPLDLink,
    keys: impl Iterator<Item = K> + IntoIterator<Item = K>,
) -> impl Stream<Item = Result<(K, V), Error>> {
    let mut batch = keys.into_iter().collect::<Vec<_>>();

    batch.sort_unstable();

    let (mut tx, rx) = mpsc::channel(batch.len());

    let tree = match ipfs.dag_get::<&str, Tree>(tree.into(), None).await {
        Ok(tree) => tree,
        Err(e) => {
            let _ = tx.try_send(Err(e.into()));
            tx.close_channel();

            return rx;
        }
    };

    execute_batch_get(ipfs.clone(), tree.root(), batch, tx).await;

    rx
} */

/* #[async_recursion]
async fn execute_batch_get<K: Key, V: Value>(
    ipfs: IpfsService,
    link: IPLDLink,
    mut batch: Vec<K>,
    mut sender: Sender<Result<(K, V), Error>>,
) {
    let node = match ipfs
        .dag_get::<&str, TreeNode<K, V>>(link.into(), None)
        .await
    {
        Ok(n) => n,
        Err(e) => {
            let _ = sender.try_send(Err(e.into()));
            return;
        }
    };

    if node.0 {
        // Find matching keys then return the KVs.
        let mut start = 0;
        for batch_key in batch {
            // Since batch and node are sorted start searching after the last index found.
            if let Ok(idx) = node.1[start..].binary_search(&batch_key) {
                let key = node.1[idx];
                let value = node.2[idx].left().unwrap();

                let _ = sender.try_send(Ok((key, value)));

                start = idx;
            }
        }
    } else {
        // Traverse to node that have keys in their range.
        let futures: Vec<_> = node
            .into_iter()
            .filter_map(|item| {
                let (range, link) = item.left().unwrap();

                let mut new_batch = Vec::with_capacity(batch.len());
                batch.retain(|batch_key| {
                    let predicate = range.contains(batch_key);

                    if predicate {
                        new_batch.push(*batch_key);
                    }

                    !predicate
                });

                if new_batch.is_empty() {
                    return None;
                }

                let future = execute_batch_get(ipfs.clone(), link, new_batch, sender.clone());

                Some(future)
            })
            .collect();

        join_all(futures).await;
    }
} */

/// Add or update values in the tree for all KVs in batch.
pub async fn batch_insert<K: Key, V: Value>(
    ipfs: IpfsService,
    root: Cid,
    strategy: Strategies,
    key_values: impl Iterator<Item = (K, V)> + IntoIterator<Item = (K, V)>,
) -> Result<Cid, Error> {
    let mut batch = key_values.into_iter().collect::<Vec<_>>();
    batch.sort_unstable_by(|(a, _), (b, _)| a.cmp(b));

    let key_links = execute_batch_insert::<K, V>(ipfs.clone(), root, strategy, batch).await?;

    if key_links.len() > 1 {
        let mut node = TreeNode::<K, Branch> {
            keys: Vec::with_capacity(key_links.len()),
            values: Branch {
                links: Vec::with_capacity(key_links.len()),
            },
        };

        node.insert(key_links.into_iter());

        let node = TreeNodes::<K, V>::Branch(node);

        let cid = ipfs.dag_put(&node, Codec::DagCbor).await?;

        return Ok(cid);
    }

    Ok(key_links[0].1)
}

#[async_recursion]
async fn execute_batch_insert<K: Key, V: Value>(
    ipfs: IpfsService,
    link: Cid,
    strategy: Strategies,
    batch: Vec<(K, V)>,
) -> Result<Vec<(K, Cid)>, Error> {
    let node = ipfs
        .dag_get::<&str, TreeNodes<K, V>>(link.into(), None)
        .await?;

    let mut nodes: Vec<TreeNodes<K, V>> = match node {
        TreeNodes::Leaf(mut leaf_node) => {
            leaf_node.insert(batch.into_iter());

            leaf_node
                .split_into(strategy)
                .into_iter()
                .map(|leaf| TreeNodes::Leaf(leaf))
                .collect()
        }
        TreeNodes::Branch(mut branch_node) => {
            let futures: Vec<_> = branch_node
                .split_batch(batch)
                .map(|(batch, link)| {
                    execute_batch_insert::<K, V>(ipfs.clone(), link, strategy, batch)
                })
                .collect();

            let results = join_all(futures).await;

            for result in results {
                let key_links = result?;

                branch_node.insert(key_links.into_iter());
            }

            branch_node
                .split_into(strategy)
                .into_iter()
                .map(|branch| TreeNodes::Branch(branch))
                .collect()
        }
    };

    let futures: Vec<_> = nodes
        .iter()
        .map(|node| {
            let ipfs = ipfs.clone();
            let node = node.clone();
            async move { ipfs.dag_put(&node, Codec::DagCbor).await }
        })
        .collect();

    let results = join_all(futures).await;

    let mut key_links = Vec::with_capacity(results.len());
    for (i, result) in results.into_iter().enumerate() {
        let cid = result?;

        let key = match &mut nodes[i] {
            TreeNodes::Branch(node) => node.keys.swap_remove(0),
            TreeNodes::Leaf(node) => node.keys.swap_remove(0),
        };

        key_links.push((key, cid));
    }

    return Ok(key_links);
}

//TODO return the value of the key removed???

/// Remove all values in the tree matching the keys.
pub async fn batch_remove<K: Key, V: Value>(
    ipfs: &IpfsService,
    root: Cid,
    keys: impl Iterator<Item = K> + IntoIterator<Item = K>,
) -> Result<Cid, Error> {
    let mut batch = keys.into_iter().collect::<Vec<_>>();
    batch.sort_unstable();

    let (_, link) = execute_batch_remove::<K, V>(ipfs.clone(), vec![root], batch).await?;

    Ok(link)
}

#[async_recursion]
async fn execute_batch_remove<K: Key, V: Value>(
    ipfs: IpfsService,
    links: Vec<Cid>,
    mut batch: Vec<K>,
) -> Result<(K, Cid), Error> {
    let futures: Vec<_> = links
        .into_iter()
        .map(|link| ipfs.dag_get::<&str, TreeNodes<K, V>>(link.into(), None))
        .collect();

    let nodes = try_join_all(futures).await?;

    // Works only because we know the nodes will be either leafs or branches.
    let mut node = nodes
        .into_iter()
        .reduce(|acc, x| match (acc, x) {
            (TreeNodes::Branch(mut node), TreeNodes::Branch(other)) => {
                node.merge(other);
                TreeNodes::Branch(node)
            }
            (TreeNodes::Leaf(mut node), TreeNodes::Leaf(other)) => {
                node.merge(other);
                TreeNodes::Leaf(node)
            }
            _ => unreachable!("The tree should always be symmetrical"),
        })
        .expect("at least one node");

    match node {
        TreeNodes::Leaf(ref mut node) => {
            node.remove_batch(batch.into_iter());
        }
        TreeNodes::Branch(ref mut node) => {
            let mut new_batch = Vec::with_capacity(batch.len());
            let mut links = Vec::with_capacity(node.keys.len());
            let mut futures = Vec::with_capacity(node.keys.len());

            'node: for i in (0..node.keys.len()).rev() {
                let key = &node.keys[i];

                let range = (
                    Bound::Included(key),
                    match node.keys.get(i + 1) {
                        Some(key) => Bound::Excluded(key),
                        None => Bound::Unbounded,
                    },
                );

                for j in (0..batch.len()).rev() {
                    let batch_key = &batch[j];

                    if Bound::Included(batch_key) == range.start_bound() {
                        node.keys.remove(i);
                        let link = node.values.links.remove(i);
                        links.push(link);

                        batch.remove(j);

                        continue 'node;
                    }

                    if range.contains(batch_key) {
                        let key = batch.remove(j);

                        new_batch.push(key);
                    }
                }

                let link = node.values.links[i];
                links.push(link);

                let future = execute_batch_remove::<K, V>(ipfs.clone(), links, new_batch);
                futures.push(future);

                new_batch = Vec::new();
                links = Vec::new();
            }

            let key_links = try_join_all(futures).await?;

            node.insert(key_links.into_iter());
        }
    }

    let cid = ipfs.dag_put(&node, Codec::DagCbor).await?;

    let key = match node {
        TreeNodes::Branch(ref mut node) => node.keys.swap_remove(0),
        TreeNodes::Leaf(ref mut node) => node.keys.swap_remove(0),
    };

    Ok((key, cid))
}

/* /// Stream all KVs in the tree in order.
pub(crate) fn stream<K: Key, V: Value>(
    ipfs: IpfsService,
    root: IPLDLink,
) -> impl Stream<Item = Result<(K, V), Error>> {
    stream::try_unfold(Some(root), move |mut root| {
        let ipfs = ipfs.clone();

        async move {
            let ipld = match root.take() {
                Some(ipld) => ipld,
                None => return Result::<_, Error>::Ok(None),
            };

            let root_node = ipfs
                .dag_get::<&str, TreeNode<K, V>>(ipld.link, None)
                .await?;

            let stream = stream_data(ipfs.clone(), root_node);

            Ok(Some((stream, root)))
        }
    })
    .try_flatten()
} */

/* fn stream_data<K: Key, V: Value>(
    ipfs: IpfsService,
    node: TreeNode<K, V>,
) -> impl Stream<Item = Result<(K, V), Error>> {
    stream::try_unfold(node.into_iter(), move |mut node_iter| {
        let ipfs = ipfs.clone();

        async move {
            let item = match node_iter.next() {
                Some(item) => item,
                None => return Result::<_, Error>::Ok(None),
            };

            match item {
                Either::Left((_, link)) => {
                    let node = ipfs
                        .dag_get::<&str, TreeNode<K, V>>(link.link, None)
                        .await?;

                    let stream = stream_data(ipfs, node).boxed_local();

                    Ok(Some((stream, node_iter)))
                }
                Either::Right((key, value)) => {
                    let stream = stream::once(async move { Ok((key, value)) }).boxed_local();

                    Ok(Some((stream, node_iter)))
                }
            }
        }
    })
    .try_flatten()
} */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_roundtrip() {
        let node = TreeNode {
            keys: vec![vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0]],
            values: Leaf {
                elements: vec![vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0]],
            },
        };

        let trenum = TreeNodes::Leaf(node);

        let encoded = serde_ipld_dagcbor::to_vec(&trenum).unwrap();
        //let decoded: TreeNodes<K, V> = serde_ipld_dagcbor::from_slice(&encoded).unwrap();

        //println!("{:?}", decoded);
    }
}

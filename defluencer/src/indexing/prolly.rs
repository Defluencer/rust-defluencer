use std::{
    fmt::Debug,
    num::NonZeroU32,
    ops::{Bound, RangeBounds},
};

use async_recursion::async_recursion;

use futures::{
    channel::mpsc::{self, Sender},
    future::join_all,
    stream, FutureExt, Stream, StreamExt, TryStreamExt,
};

use either::Either;

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::types::IPLDLink;

use crate::errors::Error;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use sha2::{Digest, Sha256};

const CHUNKING_FACTOR: u32 = u32::MAX / 16;

pub trait Key:
    Default
    + Debug
    + Clone
    + Copy
    + Eq
    + Ord
    + Serialize
    + for<'de> Deserialize<'de>
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
            + Copy
            + Eq
            + Ord
            + Serialize
            + for<'de> Deserialize<'de>
            + Send
            + Sync
            + Sized
            + AsRef<[u8]>
            + 'static,
    > Key for T
{
}

pub trait Value:
    Default + Debug + Clone + Copy + Eq + Serialize + DeserializeOwned + Send + Sync + Sized + 'static
{
}
impl<
        T: Default
            + Debug
            + Clone
            + Copy
            + Eq
            + Serialize
            + DeserializeOwned
            + Send
            + Sync
            + Sized
            + 'static,
    > Value for T
{
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct TreeNode<K, V>(bool, Vec<K>, Vec<Either<V, IPLDLink>>);

impl<K: Key, V: Value> TreeNode<K, V> {
    fn iter(&self) -> NodeIterator<K, V> {
        NodeIterator {
            node: self,
            index: 0,
        }
    }

    fn into_iter(self) -> NodeIntoIterator<K, V> {
        NodeIntoIterator {
            node: self,
            index: 0,
        }
    }

    /// Insert sorted K-Vs into this node.
    ///
    /// Idempotent.
    fn insert(
        &mut self,
        key_values: impl IntoIterator<Item = (K, Either<V, IPLDLink>)>
            + Iterator<Item = (K, Either<V, IPLDLink>)>
            + DoubleEndedIterator,
    ) {
        let mut stop = self.1.len();
        for (key, value) in key_values.rev() {
            for i in (0..stop).rev() {
                if self.1[i] < key {
                    self.1.insert(i + 1, key);
                    self.2.insert(i + 1, value);
                    stop = i + 1;
                    break;
                }

                if self.1[i] == key {
                    self.1[i] = key;
                    self.2[i] = value;
                    stop = i;
                    break;
                }
            }
        }
    }

    /// Run the chunking algorithm on this node. Return splitted nodes.
    fn split_into(self) -> Vec<Self> {
        let mut key_count = self.1.len();
        let mut value_count = self.2.len();

        let mut result = Vec::new();

        let mut node = Option::<TreeNode<K, V>>::None;
        for i in 0..self.1.len() {
            let key = self.1[i];

            let digest = Sha256::new_with_prefix(key);
            let factor = NonZeroU32::new(CHUNKING_FACTOR).unwrap(); //TODO use config
            let is_boundary = bound_check(digest, factor);

            if is_boundary {
                if let Some(node) = node.take() {
                    key_count -= node.1.len();
                    value_count -= node.2.len();

                    result.push(node);
                }

                let new_node = Self {
                    0: self.0,
                    1: Vec::with_capacity(key_count),
                    2: Vec::with_capacity(value_count),
                };

                node = Some(new_node);
            }

            // Guaranteed node because first key is a boundary
            node.as_mut().unwrap().1.push(key);

            if self.0 {
                let value = self.2[i];
                node.as_mut().unwrap().2.push(value);
            } else {
                let link = self.2[i];
                node.as_mut().unwrap().2.push(link);
            }

            if i == self.1.len() - 1 {
                let node = node.take().unwrap();
                result.push(node);
            }
        }

        result
    }

    fn merge(&mut self, other: Self) {
        self.insert(other.1.into_iter().zip(other.2.into_iter()))
    }

    fn remove_batch(
        &mut self,
        batch: impl IntoIterator<Item = K> + Iterator<Item = K> + DoubleEndedIterator,
    ) {
        let mut idx = self.1.len();
        for batch_key in batch.rev() {
            for i in (0..idx).rev() {
                let key = self.1[i];

                if batch_key == key {
                    self.1.remove(i);
                    idx = i;
                    break;
                }
            }
        }
    }
}

struct NodeIterator<'a, K: Key, V: Value> {
    node: &'a TreeNode<K, V>,
    index: usize,
}

impl<'a, K: Key, V: Value> Iterator for NodeIterator<'a, K, V> {
    type Item = Either<((Bound<&'a K>, Bound<&'a K>), &'a IPLDLink), (&'a K, &'a V)>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.node.1.len() {
            return None;
        }

        if !self.node.0 {
            let key = &self.node.1[self.index];
            let l_bound = Bound::Included(key);

            let h_bound = match self.node.1.get(self.index + 1) {
                Some(key) => Bound::Excluded(key),
                None => Bound::Unbounded,
            };

            let range = (l_bound, h_bound);
            let link = self.node.2[self.index].as_ref();

            Some(Either::Left((range, link.right().unwrap())))
        } else {
            let key = &self.node.1[self.index];
            let value = self.node.2[self.index].as_ref();

            Some(Either::Right((key, value.left().unwrap())))
        }
    }
}

struct NodeIntoIterator<K: Key, V: Value> {
    node: TreeNode<K, V>,
    index: usize,
}

impl<K: Key, V: Value> Iterator for NodeIntoIterator<K, V> {
    type Item = Either<((Bound<K>, Bound<K>), IPLDLink), (K, V)>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.node.1.len() {
            return None;
        }

        if !self.node.0 {
            let key = self.node.1[self.index];
            let l_bound = Bound::Included(key);

            let h_bound = match self.node.1.get(self.index + 1) {
                Some(key) => Bound::Excluded(*key),
                None => Bound::Unbounded,
            };

            let range = (l_bound, h_bound);
            let link = self.node.2[self.index];

            Some(Either::Left((range, link.right().unwrap())))
        } else {
            let key = self.node.1[self.index];
            let value = self.node.2[self.index];

            Some(Either::Right((key, value.left().unwrap())))
        }
    }
}

pub(crate) async fn batch_get<K: Key, V: Value>(
    ipfs: IpfsService,
    root: IPLDLink,
    mut batch: Vec<K>,
) -> impl Stream<Item = Result<(K, V), Error>> {
    batch.sort_unstable();

    let (tx, rx) = mpsc::channel(batch.len());

    execute_batch_get(ipfs.clone(), root, batch, tx).await;

    rx
}

#[async_recursion]
async fn execute_batch_get<K: Key, V: Value>(
    ipfs: IpfsService,
    link: IPLDLink,
    mut batch: Vec<K>,
    mut sender: Sender<Result<(K, V), Error>>,
) {
    let node = match ipfs.dag_get::<&str, TreeNode<K, V>>(link.link, None).await {
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

        //TODO is the KV order preserved through the channel???
        join_all(futures).await;
    }
}

pub(crate) async fn batch_insert<K: Key, V: Value>(
    ipfs: &IpfsService,
    root: IPLDLink,
    mut batch: Vec<(K, V)>,
) -> Result<IPLDLink, Error> {
    //TODO get config node

    batch.sort_unstable_by(|(a, _), (b, _)| a.cmp(b));

    let key_links = execute_batch_insert::<K, V>(ipfs.clone(), root, batch).await?;

    if key_links.len() > 1 {
        let mut node = TreeNode::<K, V> {
            0: true,
            1: Vec::with_capacity(key_links.len()),
            2: Vec::with_capacity(key_links.len()),
        };

        node.insert(
            key_links
                .into_iter()
                .map(|(key, link)| (key, Either::Right(link))),
        );

        let cid = ipfs.dag_put(&node, Codec::DagCbor).await?;
        let link: IPLDLink = cid.into();
        return Ok(link);
    }

    //TODO update config node

    Ok(key_links[0].1)
}

#[async_recursion]
async fn execute_batch_insert<K: Key, V: Value>(
    ipfs: IpfsService,
    link: IPLDLink,
    mut batch: Vec<(K, V)>,
) -> Result<Vec<(K, IPLDLink)>, Error> {
    let mut node = ipfs
        .dag_get::<&str, TreeNode<K, V>>(link.link, None)
        .await?;

    if !node.0 {
        let futures: Vec<_> = node
            .iter()
            .map(|item| {
                // Guaranteed non-leaf node because of the if statement
                let (range, link) = item.left().unwrap();

                let mut new_batch = Vec::with_capacity(batch.len());
                batch.retain(|&(key, value)| {
                    let predicate = range.contains(&key);

                    if predicate {
                        new_batch.push((key, value));
                    }

                    !predicate
                });

                execute_batch_insert::<K, V>(ipfs.clone(), *link, new_batch)
            })
            .collect();

        let results = join_all(futures).await;

        for result in results {
            let key_links = result?;

            node.insert(
                key_links
                    .into_iter()
                    .map(|(key, link)| (key, Either::Right(link))),
            );
        }
    } else {
        node.insert(
            batch
                .into_iter()
                .map(|(key, value)| (key, Either::Left(value))),
        );
    }

    let nodes = node.split_into();

    let futures: Vec<_> = nodes
        .iter()
        .map(|node| {
            let ipfs = ipfs.clone();
            let node = node.clone();
            async move { ipfs.dag_put(&node, Codec::DagCbor).await }
        })
        .collect();

    let results = join_all(futures).await;

    let mut key_2 = Vec::with_capacity(results.len());
    for (i, result) in results.into_iter().enumerate() {
        let cid = result?;

        let key = nodes[i].1[0];
        let link: IPLDLink = cid.into();
        key_2.push((key, link));
    }

    return Ok(key_2);
}

//TODO return the value of the key removed???

pub(crate) async fn batch_remove<K: Key, V: Value>(
    ipfs: &IpfsService,
    root: IPLDLink,
    mut batch: Vec<K>,
) -> Result<IPLDLink, Error> {
    //TODO get config node

    batch.sort_unstable();

    let (key, link) = execute_batch_remove::<K, V>(ipfs.clone(), vec![root], batch).await?;

    //TODO update config node

    Ok(link)
}

#[async_recursion]
async fn execute_batch_remove<K: Key, V: Value>(
    ipfs: IpfsService,
    links: Vec<IPLDLink>,
    mut batch: Vec<K>,
) -> Result<(K, IPLDLink), Error> {
    let futures: Vec<_> = links
        .into_iter()
        .map(|link| ipfs.dag_get::<&str, TreeNode<K, V>>(link.into(), None))
        .collect();

    let results = join_all(futures).await;

    let mut node = TreeNode::default();
    for result in results {
        node.merge(result?);
    }

    if !node.0 {
        let mut new_batch = Vec::with_capacity(batch.len());
        let mut links = Vec::with_capacity(node.1.len());
        let mut futures = Vec::with_capacity(node.1.len());

        'node: for i in (0..node.1.len()).rev() {
            let key = &node.1[i];

            let range = (
                Bound::Included(key),
                match node.1.get(i + 1) {
                    Some(key) => Bound::Excluded(key),
                    None => Bound::Unbounded,
                },
            );

            for j in (0..batch.len()).rev() {
                let batch_key = &batch[j];

                if Bound::Included(batch_key) == range.start_bound() {
                    node.1.remove(i);
                    let link = node.2.remove(i);
                    links.push(link.right().unwrap());

                    batch.remove(j);

                    continue 'node;
                }

                if range.contains(batch_key) {
                    let key = batch.remove(j);

                    new_batch.push(key);
                }
            }

            links.push(node.2[i].right().unwrap());

            futures.push(execute_batch_remove::<K, V>(ipfs.clone(), links, new_batch));

            new_batch = Vec::new();
            links = Vec::new();
        }

        let results = join_all(futures).await;

        for result in results {
            let (key, link) = result?;

            node.insert(vec![(key, Either::Right(link))].into_iter());
        }
    } else {
        node.remove_batch(batch.into_iter());
    }

    let key = node.1[0];
    let cid = ipfs.dag_put(&node, Codec::DagCbor).await?;
    let link: IPLDLink = cid.into();

    Ok((key, link))
}

pub(crate) fn values<K: Key, V: Value>(
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
}

fn stream_data<K: Key, V: Value>(
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
}

fn bound_check(digest: impl Digest, chunking_factor: NonZeroU32) -> bool {
    let hash = digest.finalize();

    let zero_count: u32 = hash
        .into_iter()
        .rev()
        .take(4)
        .map(|byte| byte.count_zeros())
        .sum();

    let threshold = (u32::MAX / chunking_factor.get()).count_zeros();

    zero_count > threshold
}

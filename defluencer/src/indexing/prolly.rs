use std::{
    collections::VecDeque,
    fmt::Debug,
    hash::Hash,
    num::NonZeroU32,
    ops::{Bound, RangeBounds},
};

use async_recursion::async_recursion;

use futures::future::join_all;

use either::Either::{self};

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
    + DeserializeOwned
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
            + DeserializeOwned
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

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct TreeNode<K, V> {
    is_leaf: bool,

    keys: Vec<K>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    values: Vec<V>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    links: Vec<IPLDLink>,
}

impl<K: Key, V: Value> TreeNode<K, V> {
    fn iter(&self) -> NodeIterator<K, V> {
        NodeIterator {
            node: self,
            index: 0,
        }
    }

    /// Insert sorted K-Vs into this node.
    ///
    /// Idempotent.
    fn insert_values(
        &mut self,
        key_values: impl IntoIterator<Item = (K, V)> + Iterator<Item = (K, V)> + DoubleEndedIterator,
    ) {
        let mut stop = self.keys.len();
        for (key, value) in key_values.rev() {
            for i in (0..stop).rev() {
                if self.keys[i] < key {
                    self.keys.insert(i + 1, key);
                    self.values.insert(i + 1, value);
                    stop = i + 1;
                    break;
                }

                if self.keys[i] == key {
                    self.keys[i] = key;
                    self.values[i] = value;
                    stop = i;
                    break;
                }
            }
        }
    }

    /// Insert sorted key and links into this node.
    ///
    /// Idempotent.
    fn insert_links(
        &mut self,
        key_links: impl IntoIterator<Item = (K, IPLDLink)>
            + Iterator<Item = (K, IPLDLink)>
            + DoubleEndedIterator,
    ) {
        let mut stop = self.keys.len();
        for (key, link) in key_links.rev() {
            for i in (0..stop).rev() {
                if self.keys[i] < key {
                    self.keys.insert(i + 1, key);
                    self.links.insert(i + 1, link);
                    stop = i + 1;
                    break;
                }

                if self.keys[i] == key {
                    self.keys[i] = key;
                    self.links[i] = link;
                    stop = i;
                    break;
                }
            }
        }
    }

    /// Run the chunking algorithm on this node. Return splitted nodes.
    fn split_into(self) -> Vec<Self> {
        let mut key_count = self.keys.len();
        let mut value_count = self.values.len();
        let mut link_count = self.links.len();

        let mut result = Vec::new();

        let mut node = Option::<TreeNode<K, V>>::None;
        for i in 0..self.keys.len() {
            let key = self.keys[i];

            let digest = Sha256::new_with_prefix(key);
            let factor = NonZeroU32::new(CHUNKING_FACTOR).unwrap(); //TODO use config
            let is_boundary = bound_check(digest, factor);

            if is_boundary {
                if let Some(node) = node.take() {
                    key_count -= node.keys.len();
                    value_count -= node.values.len();
                    link_count -= node.links.len();

                    result.push(node);
                }

                let new_node = Self {
                    is_leaf: self.is_leaf,
                    keys: Vec::with_capacity(key_count),
                    values: Vec::with_capacity(value_count),
                    links: Vec::with_capacity(link_count),
                };

                node = Some(new_node);
            }

            // Guaranteed node because first key is a boundary
            node.as_mut().unwrap().keys.push(key);

            if self.is_leaf {
                let value = self.values[i];
                node.as_mut().unwrap().values.push(value);
            } else {
                let link = self.links[i];
                node.as_mut().unwrap().links.push(link);
            }

            if i == self.keys.len() - 1 {
                let node = node.take().unwrap();
                result.push(node);
            }
        }

        result
    }
}

/* impl<'a, K: Key, V: Value> IntoIterator for &'a TreeNode<K, V> {
    type Item = Either<((Bound<&'a K>, Bound<&'a K>), &'a IPLDLink), (&'a K, &'a V)>;

    type IntoIter = NodeIterator<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        NodeIterator {
            node: self,
            index: 0,
        }
    }
} */

struct NodeIterator<'a, K: Key, V: Value> {
    node: &'a TreeNode<K, V>,
    index: usize,
}

impl<'a, K: Key, V: Value> Iterator for NodeIterator<'a, K, V> {
    type Item = Either<((Bound<&'a K>, Bound<&'a K>), &'a IPLDLink), (&'a K, &'a V)>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.node.keys.len() {
            return None;
        }

        if !self.node.is_leaf {
            let key = &self.node.keys[self.index];
            let l_bound = Bound::Included(key);

            let h_bound = match self.node.keys.get(self.index + 1) {
                Some(key) => Bound::Excluded(key),
                None => Bound::Unbounded,
            };

            let range = (l_bound, h_bound);
            let link = &self.node.links[self.index];

            Some(Either::Left((range, link)))
        } else {
            let key = &self.node.keys[self.index];
            let value = &self.node.values[self.index];

            Some(Either::Right((key, value)))
        }
    }
}

/* pub(crate) async fn batch_get<K: Key, V: Value>(
    ipfs: &IpfsService,
    root: IPLDLink,
    mut keys: Vec<K>,
) -> impl Stream<Item = Result<(K, V), Error>> {
    todo!()
} */

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
            is_leaf: true,
            keys: Vec::with_capacity(key_links.len()),
            values: vec![],
            links: Vec::with_capacity(key_links.len()),
        };

        node.insert_links(key_links.into_iter());

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

    if !node.is_leaf {
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

            node.insert_links(key_links.into_iter());
        }
    } else {
        node.insert_values(batch.into_iter());
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

    let mut key_links = Vec::with_capacity(results.len());
    for (i, result) in results.into_iter().enumerate() {
        let cid = result?;

        let key = nodes[i].keys[0];
        let link: IPLDLink = cid.into();
        key_links.push((key, link));
    }

    return Ok(key_links);
}

pub(crate) async fn batch_remove<K: Key, V: Value>(
    ipfs: &IpfsService,
    root: IPLDLink,
    keys: Vec<K>,
) -> Result<Option<IPLDLink>, Error> {
    todo!()
}

/* pub(crate) fn values<K: Key, V: Value>(
    ipfs: IpfsService,
    root: IPLDLink,
) -> impl Stream<Item = Result<(K, V), Error>> {
    todo!()
} */

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

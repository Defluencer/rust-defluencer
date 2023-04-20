use std::{
    fmt::Debug,
    ops::{Bound, RangeBounds},
};

use async_recursion::async_recursion;

use futures::{future::try_join_all, stream, Stream, StreamExt, TryStreamExt};

use ipfs_api::{responses::Codec, IpfsService};

use crate::errors::Error;

use super::{deserialization::TreeNodes, Config};

use cid::Cid;

use libipld_core::ipld::Ipld;

/// Trait for tree keys.
///
/// Notable bounds are; ordered by their byte representation and compatible with Ipld.
///
/// As for ```str``` and ```String``` read this std [note](https://doc.rust-lang.org/std/cmp/trait.Ord.html#impl-Ord-for-str)
pub trait Key: Clone + Ord + TryFrom<Ipld> + Into<Ipld> + Send + Sync + 'static {}
impl<T: Clone + Ord + TryFrom<Ipld> + Into<Ipld> + Send + Sync + 'static> Key for T {}

/// Trait for tree values.
///
/// Only notable bound is compatibility with Ipld.
pub trait Value: Clone + TryFrom<Ipld> + Into<Ipld> + Send + Sync + 'static {}
impl<T: Clone + TryFrom<Ipld> + Into<Ipld> + Send + Sync + 'static> Value for T {}

/// Type state for tree leaf nodes
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Leaf<V> {
    pub elements: Vec<V>,
}

/// Type state for tree branch nodes
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Branch {
    pub links: Vec<Cid>,
}

pub trait TreeNodeType {}
impl<V> TreeNodeType for Leaf<V> {}
impl TreeNodeType for Branch {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeNode<K, T: TreeNodeType> {
    pub keys: Vec<K>,
    pub values: T,
}

impl<K: Key, T: TreeNodeType> TreeNode<K, T> {
    /// Find the index in node for each key in the batch
    fn search<'a>(
        &'a self,
        batch: impl IntoIterator<Item = K> + 'a,
    ) -> impl Iterator<Item = usize> + 'a {
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

impl<K: Key> Default for TreeNode<K, Branch> {
    fn default() -> Self {
        Self {
            keys: Default::default(),
            values: Branch {
                links: Default::default(),
            },
        }
    }
}

impl<K: Key> TreeNode<K, Branch> {
    /// Insert sorted keys and links into this node.
    ///
    /// Idempotent.
    fn insert(&mut self, key_values: impl IntoIterator<Item = (K, Cid)>) {
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

    /// Split the batch into smaller batch with associated node links
    /// while ignoring batch key not present in node.
    fn search_batch_split<'a>(
        &'a self,
        batch: impl IntoIterator<Item = K> + 'a,
    ) -> impl Iterator<Item = (Vec<K>, Cid)> + 'a {
        //TODO refactor into one scan call
        batch
            .into_iter()
            .scan((self, 0usize), |(node, start), key| {
                match node.keys[*start..].binary_search(&key) {
                    Ok(idx) => {
                        let link = node.values.links[idx];

                        *start = idx;
                        return Some((key, link));
                    }
                    Err(_) => return None,
                }
            })
            .scan(
                (Option::<Vec<K>>::None, Option::<Cid>::None),
                |(batch, batch_link), (key, link)| {
                    if batch.is_none() || batch_link.is_none() {
                        *batch = Some(vec![key]);
                        *batch_link = Some(link);

                        return None;
                    }

                    if *batch_link.as_ref().unwrap() == link {
                        batch.as_mut().unwrap().push(key);

                        return None;
                    }

                    let batch = batch.take().unwrap();
                    let batch_link = batch_link.take().unwrap();

                    return Some((batch, batch_link));
                },
            )
    }

    /// Split the batch into smaller batch with associated node links.
    fn insert_batch_split<'a, V: Value>(
        &'a self,
        batch: impl IntoIterator<Item = (K, V)> + 'a,
    ) -> impl Iterator<Item = (Vec<(K, V)>, Cid)> + 'a {
        //TODO refactor into one scan call???
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

    /// Run the chunking algorithm on this node. Return splitted nodes in order if any.
    fn split_with<V: Value>(self, mut config: Config) -> Vec<Self> {
        let (bytes, mut og) = {
            let tree_nodes = TreeNodes::<K, V>::Branch(self);

            let bytes = serde_ipld_dagcbor::to_vec(&tree_nodes).expect("Tree node serialization");

            let TreeNodes::<K, V>::Branch(node) = tree_nodes else {
                unreachable!();
            };

            (bytes, node)
        };

        if bytes.len() < config.min_size {
            return vec![og];
        }

        let mut nodes = Vec::new();

        for i in (1..og.keys.len()).rev() {
            let key = &og.keys[i];
            let value = &og.values.links[i];

            if config.boundary(key.clone(), value.clone()) {
                //TODO check min size before splitting

                let keys = og.keys.split_off(i);
                let links = og.values.links.split_off(i);

                let node = TreeNode {
                    keys,
                    values: Branch { links },
                };

                let (node_bytes, mut node) = {
                    let tree_nodes = TreeNodes::<K, V>::Branch(node);
                    let bytes =
                        serde_ipld_dagcbor::to_vec(&tree_nodes).expect("Tree node serialization");
                    let TreeNodes::<K, V>::Branch(node) = tree_nodes else {
                        unreachable!();
                    };
                    (bytes, node)
                };

                if node_bytes.len() > config.max_size {
                    // Get % of bytes over the max then remove same % of KVs
                    let percent = ((node_bytes.len() - config.max_size) as f64)
                        / (config.max_size as f64)
                        * 100.0;
                    let count = ((node.keys.len() as f64) * percent) as usize;
                    let idx = node.keys.len() - count.max(1);

                    let keys = node.keys.split_off(idx);
                    let links = node.values.links.split_off(idx);

                    let new_node = TreeNode {
                        keys,
                        values: Branch { links },
                    };

                    nodes.push(new_node);
                }

                nodes.push(node);
            }
        }

        if !og.keys.is_empty() {
            nodes.push(og);
        }

        nodes.reverse();

        nodes
    }

    /// Merge all node keys and links with other
    fn merge(&mut self, other: Self) {
        self.insert(other.keys.into_iter().zip(other.values.links.into_iter()))
    }

    /// Remove key and links that match batch keys
    ///
    /// Idempotent.
    fn remove_batch(&mut self, batch: impl IntoIterator<Item = K>) {
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

    pub fn into_iter(self) -> BranchIntoIterator<K> {
        BranchIntoIterator {
            node: self,
            index: 0,
        }
    }
}

pub struct BranchIterator<'a, K> {
    node: &'a TreeNode<K, Branch>,
    index: usize,
}

impl<'a, K> Iterator for BranchIterator<'a, K> {
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

        self.index += 1;

        Some((range, link))
    }
}

pub struct BranchIntoIterator<K> {
    node: TreeNode<K, Branch>,
    index: usize,
}

impl<K: Key> Iterator for BranchIntoIterator<K> {
    type Item = ((Bound<K>, Bound<K>), Cid);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.node.keys.len() {
            return None;
        }

        let key = self.node.keys[self.index].clone();
        let l_bound = Bound::Included(key);

        let h_bound = match self.node.keys.get(self.index + 1) {
            Some(key) => Bound::Excluded(key.clone()),
            None => Bound::Unbounded,
        };

        let range = (l_bound, h_bound);
        let link = self.node.values.links[self.index];

        self.index += 1;

        Some((range, link))
    }
}

impl<K: Key, V: Value> Default for TreeNode<K, Leaf<V>> {
    fn default() -> Self {
        Self {
            keys: Default::default(),
            values: Leaf {
                elements: Default::default(),
            },
        }
    }
}

impl<K: Key, V: Value> TreeNode<K, Leaf<V>> {
    /// Insert sorted keys and values into this node.
    ///
    /// Idempotent.
    fn insert(&mut self, key_values: impl IntoIterator<Item = (K, V)>) {
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

    /// Run the chunking algorithm on this node. Return splitted nodes in order if any.
    fn split_with(self, mut config: Config) -> Vec<Self> {
        let (bytes, mut og) = {
            let tree_nodes = TreeNodes::<K, V>::Leaf(self);

            let bytes = serde_ipld_dagcbor::to_vec(&tree_nodes).expect("Tree node serialization");

            let TreeNodes::<K, V>::Leaf(node) = tree_nodes else {
                unreachable!();
            };

            (bytes, node)
        };

        if bytes.len() < config.min_size {
            return vec![og];
        }

        let mut nodes = Vec::new();

        for i in (1..og.keys.len()).rev() {
            let key = &og.keys[i];
            let value = &og.values.elements[i];

            if config.boundary(key.clone(), value.clone()) {
                let keys = og.keys.split_off(i);
                let elements = og.values.elements.split_off(i);

                let node = TreeNode {
                    keys,
                    values: Leaf { elements },
                };

                let (node_bytes, mut node) = {
                    let tree_nodes = TreeNodes::<K, V>::Leaf(node);
                    let bytes =
                        serde_ipld_dagcbor::to_vec(&tree_nodes).expect("Tree node serialization");
                    let TreeNodes::<K, V>::Leaf(node) = tree_nodes else {
                        unreachable!();
                    };
                    (bytes, node)
                };

                if node_bytes.len() > config.max_size {
                    // Get % of bytes over the max then remove same % of KVs
                    let percent = ((node_bytes.len() - config.max_size) as f64)
                        / (config.max_size as f64)
                        * 100.0;
                    let count = ((node.keys.len() as f64) * percent) as usize;
                    let idx = node.keys.len() - count.max(1);

                    let keys = node.keys.split_off(idx);
                    let elements = node.values.elements.split_off(idx);

                    let new_node = TreeNode {
                        keys,
                        values: Leaf { elements },
                    };

                    nodes.push(new_node);
                }

                nodes.push(node);
            }
        }

        if !og.keys.is_empty() {
            nodes.push(og);
        }

        nodes.reverse();

        nodes
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
    fn remove_batch(&mut self, batch: impl IntoIterator<Item = K>) {
        let mut start = 0;
        for batch_key in batch {
            if let Ok(idx) = self.keys[start..].binary_search(&batch_key) {
                self.keys.remove(idx);
                self.values.elements.remove(idx);

                start = idx;
            }
        }
    }

    pub fn iter(
        &self,
    ) -> impl IntoIterator<Item = (&K, &V)> + Iterator<Item = (&K, &V)> + DoubleEndedIterator {
        self.keys.iter().zip(self.values.elements.iter())
    }

    pub fn into_iter(
        self,
    ) -> impl IntoIterator<Item = (K, V)> + Iterator<Item = (K, V)> + DoubleEndedIterator {
        self.keys.into_iter().zip(self.values.elements.into_iter())
    }
}

/// Stream all the KVs that coorespond with the keys in batch.
pub fn batch_get<K: Key, V: Value>(
    ipfs: IpfsService,
    root: Cid,
    keys: impl IntoIterator<Item = K>,
) -> impl Stream<Item = Result<(K, V), Error>> {
    let mut batch = keys.into_iter().collect::<Vec<_>>();
    batch.sort_unstable();

    stream::once(async move {
        match ipfs.dag_get::<&str, TreeNodes<K, V>>(root, None).await {
            Ok(node) => Ok((ipfs, node, batch)),
            Err(e) => Err(e),
        }
    })
    .map_ok(|(ipfs, node, batch)| match node {
        TreeNodes::Branch(branch) => search_branch(ipfs, branch, batch).boxed_local(),
        TreeNodes::Leaf(leaf) => search_leaf(leaf, batch).boxed_local(),
    })
    .try_flatten()
}

fn search_branch<K: Key, V: Value>(
    ipfs: IpfsService,
    branch: TreeNode<K, Branch>,
    batch: impl IntoIterator<Item = K>,
) -> impl Stream<Item = Result<(K, V), Error>> {
    let batches = branch
        .search_batch_split(batch.into_iter())
        .map(|(batch, link)| Ok((ipfs.clone(), batch, link)))
        .collect::<Vec<_>>();

    stream::iter(batches.into_iter())
        .and_then(|(ipfs, batch, link)| async move {
            match ipfs.dag_get::<&str, TreeNodes<K, V>>(link, None).await {
                Ok(node) => Ok((ipfs, node, batch)),
                Err(e) => Err(e),
            }
        })
        .map_ok(|(ipfs, node, batch)| match node {
            TreeNodes::Branch(branch) => search_branch(ipfs, branch, batch).boxed_local(),
            TreeNodes::Leaf(leaf) => search_leaf(leaf, batch).boxed_local(),
        })
        .try_flatten()
}

fn search_leaf<K: Key, V: Value>(
    leaf: TreeNode<K, Leaf<V>>,
    batch: Vec<K>,
) -> impl Stream<Item = Result<(K, V), Error>> {
    //TODO is there a way to drop the unwanted KVs instead of cloning?
    let results: Vec<_> = leaf
        .search(batch.into_iter())
        .map(|index| {
            Ok((
                leaf.keys[index].clone(),
                leaf.values.elements[index].clone(),
            ))
        })
        .collect();

    stream::iter(results.into_iter())
}

/// Add or update values in the tree.
pub async fn batch_insert<K: Key, V: Value>(
    ipfs: IpfsService,
    root: Cid,
    config: Config,
    key_values: impl IntoIterator<Item = (K, V)>,
) -> Result<Cid, Error> {
    let mut batch = key_values.into_iter().collect::<Vec<_>>();
    batch.sort_unstable_by(|(a, _), (b, _)| a.cmp(b));

    let key_links = execute_batch_insert(ipfs.clone(), root, config, batch).await?;

    if key_links.len() > 1 {
        let mut node = TreeNode::<K, Branch>::default();

        node.insert(key_links.into_iter());

        let node = TreeNodes::<K, V>::Branch(node);

        let cid = ipfs.dag_put(&node, Codec::DagCbor).await?; // TODO use tree encoding config

        return Ok(cid);
    }

    Ok(key_links[0].1)
}

#[async_recursion]
async fn execute_batch_insert<K: Key, V: Value>(
    ipfs: IpfsService,
    link: Cid,
    config: Config,
    batch: Vec<(K, V)>,
) -> Result<Vec<(K, Cid)>, Error> {
    let node = ipfs
        .dag_get::<&str, TreeNodes<K, V>>(link.into(), None)
        .await?;

    let nodes: Vec<TreeNodes<K, V>> = match node {
        TreeNodes::Leaf(mut leaf_node) => {
            leaf_node.insert(batch.into_iter());

            leaf_node
                .split_with(config.clone())
                .into_iter()
                .map(|leaf| TreeNodes::Leaf(leaf))
                .collect()
        }
        TreeNodes::Branch(mut branch_node) => {
            let futures: Vec<_> = branch_node
                .insert_batch_split(batch)
                .map(|(batch, link)| {
                    execute_batch_insert(ipfs.clone(), link, config.clone(), batch)
                })
                .collect();

            let key_links = try_join_all(futures).await?;

            branch_node.insert(key_links.into_iter().flatten());

            branch_node
                .split_with::<V>(config)
                .into_iter()
                .map(|branch| TreeNodes::Branch(branch))
                .collect()
        }
    };

    let keys = nodes
        .iter()
        .map(|node| match node {
            TreeNodes::Branch(node) => node.keys[0].clone(),
            TreeNodes::Leaf(node) => node.keys[0].clone(),
        })
        .collect::<Vec<_>>();

    let futures: Vec<_> = nodes
        .into_iter()
        .map(|node| {
            let ipfs = ipfs.clone();

            async move { ipfs.dag_put(&node, Codec::DagCbor).await } // TODO use tree encoding config
        })
        .collect();

    let links = try_join_all(futures).await?;

    let key_links = keys.into_iter().zip(links.into_iter()).collect();

    return Ok(key_links);
}

//TODO return the value of the key removed???

/// Remove all values in the tree matching the keys.
pub async fn batch_remove<K: Key, V: Value>(
    ipfs: IpfsService,
    root: Cid,
    keys: impl IntoIterator<Item = K>,
) -> Result<Cid, Error> {
    let mut batch = keys.into_iter().collect::<Vec<_>>();
    batch.sort_unstable();

    let (_, link) = execute_batch_remove::<K, V>(ipfs, vec![root], batch).await?;

    Ok(link)
}

#[async_recursion]
async fn execute_batch_remove<K: Key, V: Value>(
    ipfs: IpfsService,
    links: Vec<Cid>,
    mut batch: Vec<K>,
) -> Result<(K, Cid), Error> {
    let futures = links
        .into_iter()
        .map(|link| ipfs.dag_get::<&str, TreeNodes<K, V>>(link, None))
        .collect::<Vec<_>>();

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
            // Create futures that will merge the nodes at the links.

            let mut new_batch = Vec::with_capacity(batch.len());
            let mut links = Vec::with_capacity(node.keys.len());
            let mut futures = Vec::with_capacity(node.keys.len());

            // For each node keys
            'node: for i in (0..node.keys.len()).rev() {
                let key = &node.keys[i];

                // Get a range for this key (In a branch node, values are links to other nodes).
                let range = (
                    Bound::Included(key),
                    match node.keys.get(i + 1) {
                        Some(key) => Bound::Excluded(key),
                        None => Bound::Unbounded,
                    },
                );

                // For each key in batch,
                for j in (0..batch.len()).rev() {
                    let batch_key = &batch[j];

                    // if the batch key is the first key in the range
                    if Bound::Included(batch_key) == range.start_bound() {
                        // remove it from this node,
                        node.keys.remove(i);
                        // remove the key from the batch,
                        batch.remove(j);

                        // remove the link
                        let link = node.values.links.remove(i);
                        // and save it.
                        links.push(link);

                        // Continue with next node key.
                        continue 'node;
                    }

                    // If the batch key is in range
                    if range.contains(batch_key) {
                        // remove it from this batch
                        let key = batch.remove(j);
                        // and save it.
                        new_batch.push(key);
                    }
                }

                // Get the link for this node key
                let link = node.values.links[i];
                // and save it.
                links.push(link);

                // Recurse
                let future = execute_batch_remove::<K, V>(ipfs.clone(), links, new_batch);
                futures.push(future);

                // Clear the saved batch keys and links
                new_batch = Vec::new();
                links = Vec::new();
            }

            let key_links = try_join_all(futures).await?;

            node.insert(key_links.into_iter());
        }
    }

    let cid = ipfs.dag_put(&node, Codec::DagCbor).await?; // TODO use tree encoding config

    let key = match node {
        TreeNodes::Branch(ref mut node) => node.keys.swap_remove(0),
        TreeNodes::Leaf(ref mut node) => node.keys.swap_remove(0),
    };

    Ok((key, cid))
}

/// Stream all KVs in the tree in order.
pub fn stream_pairs<K: Key, V: Value>(
    ipfs: IpfsService,
    root: Cid,
) -> impl Stream<Item = Result<(K, V), Error>> {
    let ipfs_clone = ipfs.clone();

    stream::once(async move { ipfs.dag_get::<&str, TreeNodes<K, V>>(root, None).await })
        .map_ok(move |node| {
            let ipfs = ipfs_clone.clone();

            match node {
                TreeNodes::Branch(branch) => stream_branch(ipfs, branch).boxed_local(),
                TreeNodes::Leaf(leaf) => {
                    stream::iter(leaf.into_iter().map(|item| Ok(item))).boxed_local()
                }
            }
        })
        .try_flatten()
}

fn stream_branch<K: Key, V: Value>(
    ipfs: IpfsService,
    branch: TreeNode<K, Branch>,
) -> impl Stream<Item = Result<(K, V), Error>> {
    let ipfs_clone = ipfs.clone();

    stream::iter(branch.into_iter())
        .map(|(_, link)| Ok(link))
        .and_then(move |link| {
            let ipfs = ipfs_clone.clone();

            async move { ipfs.dag_get::<&str, TreeNodes<K, V>>(link, None).await }
        })
        .map_ok(move |node| {
            let ipfs = ipfs.clone();

            match node {
                TreeNodes::Branch(branch) => stream_branch(ipfs, branch).boxed_local(),
                TreeNodes::Leaf(leaf) => stream::iter(leaf.into_iter())
                    .map(|item| Ok(item))
                    .boxed_local(),
            }
        })
        .try_flatten()
}

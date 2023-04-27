use std::{collections::VecDeque, fmt::Debug};

use super::{
    deserialization::TreeNodes,
    iterators::{BranchIntoIterator, BranchIterator, Insert, Remove, Search},
    Config,
};

use cid::Cid;

use crate::indexing::ordered_trees::{
    errors::Error,
    traits::{Key, Value},
};

/// Type state for tree leaf nodes
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Leaf<V> {
    pub elements: Vec<V>,
}

/// Type state for tree branch nodes
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Branch {
    pub links: VecDeque<Cid>,
}

pub trait TreeNodeType {}
impl<V> TreeNodeType for Leaf<V> {}
impl TreeNodeType for Branch {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeNode<K, T: TreeNodeType> {
    pub keys: VecDeque<K>,
    pub values: T,
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
    pub fn insert(&mut self, key_values: impl IntoIterator<Item = (K, Cid)>) {
        for (key, value) in key_values {
            match self.keys.binary_search(&key) {
                Ok(idx) => {
                    self.keys[idx] = key;
                    self.values.links[idx] = value;
                }
                Err(idx) => {
                    self.keys.insert(idx, key);
                    self.values.links.insert(idx, value);
                }
            }
        }
    }

    /// Split the batch into smaller batch with associated node links
    pub fn search_batch<'a>(
        &'a self,
        batch: impl IntoIterator<Item = K> + 'a,
    ) -> impl Iterator<Item = (Cid, Vec<K>)> + 'a {
        Search {
            node: self,
            batch: batch.into_iter().collect(),
        }
    }

    /// Split the batch into smaller batch with associated node links.
    pub fn insert_batch<'a, V: Value>(
        &'a self,
        batch: impl IntoIterator<Item = (K, V)> + 'a,
    ) -> impl Iterator<Item = (Cid, Vec<(K, V)>)> + 'a {
        Insert {
            node: self,
            batch: batch.into_iter().collect(),
        }
    }

    /// Split the batch into smaller batch with associated node links while removing batch keys.
    pub fn remove_batch<'a, V: Value>(
        &'a mut self,
        batch: impl IntoIterator<Item = K> + 'a,
    ) -> impl Iterator<Item = (Vec<Cid>, Vec<K>)> + 'a {
        Remove {
            node: self,
            batch: batch.into_iter().collect(),
        }
    }

    /// Run the chunking algorithm on this node. Return splitted nodes in order if any.
    pub fn split_with<V: Value>(self, mut config: Config) -> Result<Vec<Self>, Error> {
        let (bytes, mut og) = {
            let tree_nodes = TreeNodes::<K, V>::Branch(self);
            let bytes = serde_ipld_dagcbor::to_vec(&tree_nodes)?;
            let TreeNodes::<K, V>::Branch(node) = tree_nodes else {
                unreachable!();
            };
            (bytes, node)
        };

        if bytes.len() < config.min_size {
            return Ok(vec![og]);
        }

        let mut nodes = Vec::new();

        for i in (1..og.keys.len()).rev() {
            let key = &og.keys[i];
            let value = &og.values.links[i];

            if config.boundary(key.clone(), value.clone())? {
                let keys = og.keys.split_off(i);
                let links = og.values.links.split_off(i);

                let node = TreeNode {
                    keys,
                    values: Branch { links },
                };

                let (node_bytes, mut node) = {
                    let tree_nodes = TreeNodes::<K, V>::Branch(node);
                    let bytes = serde_ipld_dagcbor::to_vec(&tree_nodes)?;
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

        Ok(nodes)
    }

    /// Merge all node keys and links with other
    pub fn merge(&mut self, other: Self) {
        self.insert(other.keys.into_iter().zip(other.values.links.into_iter()))
    }

    /* /// Remove key and links that match batch keys
    ///
    /// Idempotent.
    pub fn remove_batch(&mut self, batch: impl IntoIterator<Item = K>) {
        let mut start = 0;
        for batch_key in batch {
            if let Ok(idx) = self.keys[start..].binary_search(&batch_key) {
                self.keys.remove(idx);
                self.values.links.remove(idx);

                start = idx;
            }
        }
    } */

    pub fn iter(&self) -> BranchIterator<K> {
        BranchIterator {
            node: self,
            index: 0,
        }
    }

    pub fn into_iter(self) -> BranchIntoIterator<K> {
        BranchIntoIterator { node: self }
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
    pub fn insert(&mut self, key_values: impl IntoIterator<Item = (K, V)>) {
        for (key, value) in key_values {
            match self.keys.binary_search(&key) {
                Ok(idx) => {
                    self.keys[idx] = key;
                    self.values.elements[idx] = value;
                }
                Err(idx) => {
                    self.keys.insert(idx, key);
                    self.values.elements.insert(idx, value);
                }
            }
        }
    }

    /// Run the chunking algorithm on this node. Return splitted nodes in order if any.
    ///
    /// Idempotent
    pub fn split_with(self, mut config: Config) -> Result<Vec<Self>, Error> {
        let (bytes, mut og) = {
            let tree_nodes = TreeNodes::<K, V>::Leaf(self);
            let bytes = serde_ipld_dagcbor::to_vec(&tree_nodes)?;
            let TreeNodes::<K, V>::Leaf(node) = tree_nodes else {
                unreachable!();
            };
            (bytes, node)
        };

        if bytes.len() < config.min_size {
            return Ok(vec![og]);
        }

        let mut nodes = Vec::new();

        for i in (1..og.keys.len()).rev() {
            let key = &og.keys[i];
            let value = &og.values.elements[i];

            if config.boundary(key.clone(), value.clone())? {
                let keys = og.keys.split_off(i);
                let elements = og.values.elements.split_off(i);

                let node = TreeNode {
                    keys,
                    values: Leaf { elements },
                };

                let (node_bytes, mut node) = {
                    let tree_nodes = TreeNodes::<K, V>::Leaf(node);
                    let bytes = serde_ipld_dagcbor::to_vec(&tree_nodes)?;
                    let TreeNodes::<K, V>::Leaf(node) = tree_nodes else {
                        unreachable!();
                    };
                    (bytes, node)
                };

                if node_bytes.len() > config.max_size {
                    // Get % of bytes over the max then remove same % of KVs minimum of 1
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

        Ok(nodes)
    }

    /// Merge all node elements with other
    ///
    /// Idempotent
    pub fn merge(&mut self, other: Self) {
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
    pub fn remove_batch(&mut self, batch: impl IntoIterator<Item = K>) {
        for batch_key in batch {
            if let Ok(idx) = self.keys.binary_search(&batch_key) {
                self.keys.remove(idx);
                self.values.elements.remove(idx);
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

    /// Remove all KVs not present in batch
    pub fn into_search_batch(&mut self, batch: impl IntoIterator<Item = K>) {
        let mut batch_iter = batch.into_iter();
        let mut i = 0;
        while let Some(batch_key) = batch_iter.next() {
            if let Ok(idx) = self.keys.binary_search(&batch_key) {
                self.keys.swap(i, idx);
                self.values.elements.swap(i, idx);

                i += 1;
            }
        }

        self.keys.truncate(i);
        self.values.elements.truncate(i);
    }
}

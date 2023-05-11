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
    pub fn split<V: Value>(self, mut config: Config) -> Result<Vec<Self>, Error> {
        let (bytes, mut node) = {
            let node = TreeNodes::<K, V>::Branch(self);
            let bytes = serde_ipld_dagcbor::to_vec(&node)?;
            let TreeNodes::<K, V>::Branch(node) = node else {
                unreachable!();
            };
            (bytes, node)
        };

        if bytes.len() < config.min_size {
            return Ok(vec![node]);
        }

        let max_key_count = {
            // Watch out for floating point arithmtic stability
            let mult = config.max_size as f64 / bytes.len() as f64;
            (node.keys.len() as f64 * mult).floor() as usize
        };

        let capacity = (node.keys.len() as f64 / max_key_count as f64).ceil() as usize;

        let mut nodes = Vec::with_capacity(capacity);

        // Skip index 1 since we already know it's a boundary
        for i in (1..node.keys.len()).rev() {
            let key = node.keys[i].clone();
            let value = node.values.links[i];

            if config.boundary(key, value)? {
                let keys = node.keys.split_off(i);
                let links = node.values.links.split_off(i);

                let mut new_node = TreeNode {
                    keys,
                    values: Branch { links },
                };

                if new_node.keys.len() > max_key_count {
                    let idx = new_node.keys.len() - (new_node.keys.len() - max_key_count);

                    let keys = new_node.keys.split_off(idx);
                    let links = new_node.values.links.split_off(idx);

                    let split_node = TreeNode {
                        keys,
                        values: Branch { links },
                    };

                    nodes.push(split_node);
                }

                nodes.push(new_node);
            }
        }

        if !node.keys.is_empty() {
            if node.keys.len() > max_key_count {
                let idx = node.keys.len() - (node.keys.len() - max_key_count);

                let keys = node.keys.split_off(idx);
                let links = node.values.links.split_off(idx);

                let split_node = TreeNode {
                    keys,
                    values: Branch { links },
                };

                nodes.push(split_node);
            }

            nodes.push(node);
        }

        nodes.reverse();

        Ok(nodes)
    }

    /// Merge all node keys and links with other
    pub fn merge(&mut self, other: Self) {
        self.insert(other.keys.into_iter().zip(other.values.links.into_iter()))
    }

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
        let (bytes, mut node) = {
            let node = TreeNodes::<K, V>::Leaf(self);
            let bytes = serde_ipld_dagcbor::to_vec(&node)?;
            let TreeNodes::<K, V>::Leaf(node) = node else {
                unreachable!();
            };
            (bytes, node)
        };

        if bytes.len() < config.min_size {
            return Ok(vec![node]);
        }

        let max_key_count = {
            // Watch out for floating point arithmtic stability
            let mult = config.max_size as f64 / bytes.len() as f64;
            (node.keys.len() as f64 * mult).floor() as usize
        };

        let capacity = (node.keys.len() as f64 / max_key_count as f64).ceil() as usize;

        let mut nodes = Vec::with_capacity(capacity);

        for i in (1..node.keys.len()).rev() {
            let key = node.keys[i].clone();
            let value = node.values.elements[i].clone();

            if config.boundary(key, value)? {
                let keys = node.keys.split_off(i);
                let elements = node.values.elements.split_off(i);

                let mut new_node = TreeNode {
                    keys,
                    values: Leaf { elements },
                };

                if new_node.keys.len() > max_key_count {
                    let idx = new_node.keys.len() - (new_node.keys.len() - max_key_count);

                    let keys = new_node.keys.split_off(idx);
                    let elements = new_node.values.elements.split_off(idx);

                    let split_node = TreeNode {
                        keys,
                        values: Leaf { elements },
                    };

                    nodes.push(split_node);
                }

                nodes.push(new_node);
            }
        }

        if !node.keys.is_empty() {
            if node.keys.len() > max_key_count {
                let idx = node.keys.len() - (node.keys.len() - max_key_count);

                let keys = node.keys.split_off(idx);
                let elements = node.values.elements.split_off(idx);

                let split_node = TreeNode {
                    keys,
                    values: Leaf { elements },
                };

                nodes.push(split_node);
            }

            nodes.push(node);
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

#[cfg(test)]
mod tests {
    use crate::indexing::ordered_trees::prolly::{HashThreshold, Strategies};

    use super::*;

    use ipfs_api::responses::Codec;
    use rand_core::RngCore;

    use rand_xoshiro::{rand_core::SeedableRng, Xoshiro256StarStar};

    use sha2::{Digest, Sha512};

    use cid::Cid;

    use multihash::{Code, Multihash};

    #[test]
    fn into_search_batch() {
        let mut rng = Xoshiro256StarStar::from_entropy();

        let keys = VecDeque::from(vec![0, 3, 5, 7, 9, 10]);

        let elements: Vec<_> = (0..keys.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();

        let mut node = TreeNode::<u8, Leaf<Cid>> {
            keys,
            values: Leaf { elements },
        };

        let batch = vec![0, 1, 2, 5, 7, 8, 9, 10];

        node.into_search_batch(batch.clone());

        assert_eq!(node.keys.len(), 5);

        for (key, _) in node.into_iter() {
            assert!(batch.binary_search(&key).is_ok(), "Key not found in batch");
        }
    }

    #[test]
    fn split_min_size() {
        /* setup */
        let mut rng = Xoshiro256StarStar::from_entropy();

        let mut keys = VecDeque::from(vec![0, 3, 5, 1771949, 1771950, 1771951]);
        let mut elements: Vec<_> = (0..keys.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();

        let mut config = Config::default();
        config.chunking_strategy = Strategies::Threshold(HashThreshold {
            chunking_factor: 16,
            multihash_code: Code::Sha2_256,
        });
        config.codec = Codec::DagCbor;

        let split_key = 1771948u32;
        let split_value = Cid::try_from("bafyrgqaz2vrkx2tiwtfsog5wuogn4sbzxgp7o3k5654v2lqilkmtcuy74sqphrbcnykgf2yqmqpa3kreqdryqgp6dq2qqxxmtye5fuq7qvc5o").unwrap();

        keys.insert(3, split_key);
        elements.insert(3, split_value);
        let links: VecDeque<_> = elements.clone().into();

        let leaf_node = TreeNode::<u32, Leaf<Cid>> {
            keys: keys.clone(),
            values: Leaf {
                elements: elements.clone(),
            },
        };
        let branch_node = TreeNode::<u32, Branch> {
            keys: keys.clone(),
            values: Branch {
                links: links.clone(),
            },
        };

        config.min_size = 1000;

        /* execute */

        let leaf_nodes = leaf_node
            .clone()
            .split_with(config.clone())
            .expect("Node split");
        let branch_nodes = branch_node
            .clone()
            .split::<Cid>(config.clone())
            .expect("Node split");

        /* results */

        assert_eq!(leaf_nodes.len(), 1);
        assert_eq!(branch_nodes.len(), 1);
        assert_eq!(leaf_nodes[0], leaf_node);
        assert_eq!(branch_nodes[0], branch_node);
    }

    #[test]
    fn split() {
        /* setup */
        let mut rng = Xoshiro256StarStar::from_entropy();

        let mut keys = VecDeque::from(vec![0, 3, 5, 1771949, 1771950, 1771951]);
        let mut elements: Vec<_> = (0..keys.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();

        let mut config = Config::default();
        config.chunking_strategy = Strategies::Threshold(HashThreshold {
            chunking_factor: 16,
            multihash_code: Code::Sha2_256,
        });
        config.codec = Codec::DagCbor;
        config.min_size = 0;
        config.max_size = 1048576;

        let split_key = 1771948u32;
        let split_value = Cid::try_from("bafyrgqaz2vrkx2tiwtfsog5wuogn4sbzxgp7o3k5654v2lqilkmtcuy74sqphrbcnykgf2yqmqpa3kreqdryqgp6dq2qqxxmtye5fuq7qvc5o").unwrap();

        keys.insert(3, split_key);
        elements.insert(3, split_value);
        let mut links: VecDeque<_> = elements.clone().into();

        let leaf_node = TreeNode::<u32, Leaf<Cid>> {
            keys: keys.clone(),
            values: Leaf {
                elements: elements.clone(),
            },
        };
        let branch_node = TreeNode::<u32, Branch> {
            keys: keys.clone(),
            values: Branch {
                links: links.clone(),
            },
        };

        /* execute */

        let leaf_nodes = leaf_node
            .clone()
            .split_with(config.clone())
            .expect("Node split");
        let branch_nodes = branch_node
            .clone()
            .split::<Cid>(config.clone())
            .expect("Node split");

        /* results */

        assert_eq!(leaf_nodes.len(), 2);
        assert_eq!(branch_nodes.len(), 2);

        let later_keys = keys.split_off(3);
        let second_branch_node = TreeNode::<u32, Branch> {
            keys: later_keys.clone(),
            values: Branch {
                links: links.split_off(3),
            },
        };
        let second_leaf_node = TreeNode::<u32, Leaf<Cid>> {
            keys: later_keys,
            values: Leaf {
                elements: elements.split_off(3),
            },
        };

        let first_branch_node = TreeNode::<u32, Branch> {
            keys: keys.clone(),
            values: Branch {
                links: links.clone(),
            },
        };
        let first_leaf_node = TreeNode::<u32, Leaf<Cid>> {
            keys: keys.clone(),
            values: Leaf {
                elements: elements.clone(),
            },
        };

        assert_eq!(leaf_nodes[0], first_leaf_node);
        assert_eq!(leaf_nodes[1], second_leaf_node);
        assert_eq!(branch_nodes[0], first_branch_node);
        assert_eq!(branch_nodes[1], second_branch_node);
    }

    #[test]
    fn split_max_size() {
        /* setup */
        let mut rng = Xoshiro256StarStar::from_entropy();

        let mut keys = VecDeque::from(vec![0, 3, 5, 1771949, 1771950, 1771951, 1771952]);
        let mut elements: Vec<_> = (0..keys.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();
        let mut links: VecDeque<_> = elements.clone().into();

        let mut config = Config::default();
        config.chunking_strategy = Strategies::Threshold(HashThreshold {
            chunking_factor: 16,
            multihash_code: Code::Sha2_256,
        });
        config.codec = Codec::DagCbor;

        let leaf_node = TreeNode::<u32, Leaf<Cid>> {
            keys: keys.clone(),
            values: Leaf {
                elements: elements.clone(),
            },
        };
        let branch_node = TreeNode::<u32, Branch> {
            keys: keys.clone(),
            values: Branch {
                links: links.clone(),
            },
        };

        config.min_size = 0;
        config.max_size = 530;

        /* execute */

        let leaf_nodes = leaf_node
            .clone()
            .split_with(config.clone())
            .expect("Node split");
        let branch_nodes = branch_node
            .clone()
            .split::<Cid>(config.clone())
            .expect("Node split");

        /* results */

        assert_eq!(leaf_nodes.len(), 2);
        assert_eq!(branch_nodes.len(), 2);

        let last_keys = keys.split_off(6);
        let last_elements = elements.split_off(6);
        let last_links = links.split_off(6);

        let last_branch_node = TreeNode::<u32, Branch> {
            keys: last_keys.clone(),
            values: Branch { links: last_links },
        };
        let last_leaf_node = TreeNode::<u32, Leaf<Cid>> {
            keys: last_keys,
            values: Leaf {
                elements: last_elements,
            },
        };

        let first_branch_node = TreeNode::<u32, Branch> {
            keys: keys.clone(),
            values: Branch { links },
        };
        let first_leaf_node = TreeNode::<u32, Leaf<Cid>> {
            keys,
            values: Leaf { elements },
        };

        assert_eq!(leaf_nodes[0], first_leaf_node);
        assert_eq!(leaf_nodes[1], last_leaf_node);
        assert_eq!(branch_nodes[0], first_branch_node);
        assert_eq!(branch_nodes[1], last_branch_node);
    }

    fn random_cid(rng: &mut Xoshiro256StarStar) -> Cid {
        let mut input = [0u8; 64];
        rng.fill_bytes(&mut input);

        let hash = Sha512::new_with_prefix(input).finalize();

        let multihash = Multihash::wrap(0x13, &hash).unwrap();

        Cid::new_v1(/* DAG-CBOR */ 0x71, multihash)
    }
}

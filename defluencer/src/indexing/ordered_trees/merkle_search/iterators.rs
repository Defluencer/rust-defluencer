use std::{collections::VecDeque, ops::Bound};

use cid::Cid;
use either::Either;

use crate::indexing::ordered_trees::traits::{Key, Value};

use super::node::TreeNode;

impl<K: Key, V: Value> IntoIterator for TreeNode<K, V> {
    type Item = Either<(Cid, (Bound<K>, Bound<K>)), (K, V)>;

    type IntoIter = NodeIterator<K, V>;

    fn into_iter(self) -> Self::IntoIter {
        NodeIterator {
            node: self,
            index: 0,
        }
    }
}

pub struct NodeIterator<K, V> {
    pub node: TreeNode<K, V>,
    pub index: usize,
}

impl<'a, K: Key, V: Value> Iterator for NodeIterator<K, V> {
    type Item = Either<(Cid, (Bound<K>, Bound<K>)), (K, V)>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(link_idx) = self.node.indexes.front() {
            if *link_idx == self.index {
                let link_idx = self.node.indexes.pop_front().unwrap();

                let l_bound = {
                    if self.index == 0 {
                        Bound::Unbounded
                    } else {
                        match self.node.keys.get(link_idx - 1) {
                            Some(key) => Bound::Excluded(key.clone()),
                            None => Bound::Unbounded,
                        }
                    }
                };

                let h_bound = match self.node.keys.get(link_idx) {
                    Some(key) => Bound::Excluded(key.clone()),
                    None => Bound::Unbounded,
                };

                let range = (l_bound, h_bound);

                let link = self.node.links.pop_front().unwrap();

                return Some(Either::Left((link, range)));
            }
        }

        if self.node.keys.is_empty() {
            return None;
        }

        let key = self.node.keys.pop_front().unwrap();
        let value = self.node.values.pop_front().unwrap();

        self.index += 1;

        Some(Either::Right((key, value)))
    }
}

pub struct Search<K, V> {
    pub node: TreeNode<K, V>,
    pub batch: VecDeque<K>,
    pub offset: usize,
}

// Return either smaller batches or the KVs searched for.
impl<'a, K: Key, V: Value> Iterator for Search<K, V> {
    type Item = Either<(Cid, Vec<K>), (K, V)>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut keys = Vec::new();
        let mut link = None;
        while let Some(batch_key) = self.batch.front() {
            match self.node.keys.binary_search(batch_key) {
                Ok(idx) => {
                    // Return the KV found

                    if let Some(link) = link {
                        // but not if we stored keys previously, return them instead.
                        return Some(Either::Left((link, keys)));
                    }

                    self.batch.pop_front().unwrap();

                    let key = self.node.keys.swap_remove_front(idx).unwrap();
                    let value = self.node.values.swap_remove_front(idx).unwrap();

                    self.offset += 1;

                    return Some(Either::Right((key, value)));
                }
                Err(idx) => {
                    // Removing KVs in earlier steps resulted in shifted indexes.
                    let idx = idx + self.offset;

                    let batch_key = self.batch.pop_front().unwrap();

                    if let Ok(idx) = self.node.indexes.binary_search(&idx) {
                        // Found matching link

                        if link.is_none() {
                            link = Some(self.node.links[idx]);
                        };

                        keys.push(batch_key);
                    }

                    // No link found
                }
            }
        }

        None
    }
}

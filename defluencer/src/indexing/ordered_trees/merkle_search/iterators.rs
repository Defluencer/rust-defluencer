use std::{
    collections::{HashSet, VecDeque},
    ops::{Bound, RangeBounds},
};

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

impl<K: Key, V: Value> Iterator for NodeIterator<K, V> {
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

impl<K: Key, V: Value> Iterator for Search<K, V> {
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

                    // Messing the ordering is fine since we drop the node after anyway.
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

pub struct Insert<'a, K, V> {
    pub node: &'a mut TreeNode<K, V>,
    pub batch: VecDeque<(K, V, usize)>,
    pub outdated_link_idx: Vec<usize>,
}

impl<'a, K: Key, V: Value> Iterator for Insert<'a, K, V> {
    type Item = (Option<Cid>, (Bound<K>, Bound<K>), Vec<(K, V, usize)>);

    fn next(&mut self) -> Option<Self::Item> {
        let mut batch = Vec::new();
        let mut link = None;
        while let Some((batch_key, _, batch_layer)) = self.batch.front() {
            let (idx, found) = match self.node.keys.binary_search(batch_key) {
                Ok(idx) => (idx, true),
                Err(idx) => (idx, false),
            };

            if *batch_layer != self.node.layer {
                if link.is_none() {
                    if let Ok(link_idx) = self.node.indexes.binary_search(&idx) {
                        self.outdated_link_idx.push(link_idx);

                        let cid = self.node.links[link_idx];
                        link = Some(cid)
                    }
                }

                let item = self.batch.pop_front().unwrap();
                batch.push(item);
                continue;
            }

            if batch.len() > 0 {
                let lb = Bound::Excluded(self.node.keys[idx - 1].clone());
                let hb = Bound::Excluded(batch_key.clone());
                let range = (lb, hb);

                return Some((link, range, batch));
            }

            let (key, value, _) = self.batch.pop_front().unwrap();

            if found {
                self.node.values[idx] = value;
            } else {
                self.node.keys.insert(idx, key);
                self.node.values.insert(idx, value);

                let idx = self.node.indexes.binary_search(&idx).unwrap_or_else(|x| x);
                // Shift all the indexes after this add
                self.node
                    .indexes
                    .iter_mut()
                    .skip(idx)
                    .for_each(|index| *index += 1);
            }
        }

        for idx in self.outdated_link_idx.iter() {
            self.node.indexes.remove(*idx);
            self.node.links.remove(*idx);
        }

        None
    }
}

pub struct Remove<'a, K, V> {
    pub node: &'a mut TreeNode<K, V>,
    pub node_range: (Bound<K>, Bound<K>),
    pub batch: VecDeque<K>,
}

impl<'a, K: Key, V: Value> Iterator for Remove<'a, K, V> {
    type Item = (HashSet<Cid>, (Bound<K>, Bound<K>), Vec<K>);

    fn next(&mut self) -> Option<Self::Item> {
        let mut key_index = None;
        let mut range: Option<(Bound<K>, Bound<K>)> = None;
        let mut links = HashSet::new();
        let mut batch = Vec::new();
        while let Some(batch_key) = self.batch.front() {
            let (key_idx, key_found) = match self.node.keys.binary_search(batch_key) {
                Ok(idx) => (idx, true),
                Err(idx) => (idx, false),
            };

            if key_found {
                self.batch.pop_front().unwrap();

                self.node.keys.remove(key_idx);
                self.node.values.remove(key_idx);

                let link_idx = self
                    .node
                    .indexes
                    .binary_search(&key_idx)
                    .unwrap_or_else(|x| x);
                // Shift all the link indexes after this remove
                self.node
                    .indexes
                    .iter_mut()
                    .skip(link_idx)
                    .for_each(|index| *index -= 1);

                continue;
            }

            let link_idx = match self.node.indexes.binary_search(&key_idx) {
                Ok(idx) => idx,
                Err(_) => {
                    // Didn't find a link
                    let key = self.batch.pop_front().unwrap();

                    // Also check if the key fit in the batch range
                    if range.is_some() && range.as_ref().unwrap().contains(&key) {
                        batch.push(key);
                    }

                    continue;
                }
            };

            // Since we shift all key index in link indexes
            // some will have the same key index
            // that means we need to add those nodes to the same batch

            if key_index != Some(key_idx) && range.is_some() {
                // Previous batch is not the same
                return Some((links, range.unwrap(), batch));
            }

            key_index = Some(key_idx);

            let key = self.batch.pop_front().unwrap();
            self.node.indexes.remove(link_idx);
            let cid = self.node.links.remove(link_idx).unwrap();
            let lb = if key_idx == 0 {
                self.node_range.start_bound().cloned()
            } else {
                Bound::Excluded(self.node.keys[key_idx - 1].clone())
            };
            let hb = if key_idx == self.node.keys.len() {
                self.node_range.end_bound().cloned()
            } else {
                Bound::Excluded(self.node.keys[key_idx].clone())
            };

            range = Some((lb, hb));
            links.insert(cid);
            batch.push(key);
        }

        None
    }
}

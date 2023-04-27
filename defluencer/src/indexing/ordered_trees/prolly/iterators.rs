use std::{
    collections::VecDeque,
    ops::{Bound, RangeBounds},
};

use super::node::{Branch, TreeNode, TreeNodeType};

use cid::Cid;

use crate::indexing::ordered_trees::traits::{Key, Value};

pub struct Search<'a, K, T>
where
    T: TreeNodeType,
{
    pub node: &'a TreeNode<K, T>,
    pub batch: VecDeque<K>,
}

// Split the batch into smaller batches with associated node links
impl<'a, K: Key> Iterator for Search<'a, K, Branch> {
    type Item = (Cid, Vec<K>);

    fn next(&mut self) -> Option<Self::Item> {
        let mut keys = Vec::new();
        let mut link = None;
        while let Some(batch_key) = self.batch.pop_front() {
            let idx = match self.node.keys.binary_search(&batch_key) {
                Ok(idx) => idx,
                Err(idx) => idx - 1, // Since links are ordered, the previous one has the correct range.
            };

            let Some(link) = link else {
                keys.push(batch_key);
                link = Some(self.node.values.links[idx]);
                continue;
            };

            if link != self.node.values.links[idx] {
                return Some((link, keys));
            }

            keys.push(batch_key);
        }

        None
    }
}

pub struct Insert<'a, K, V, T>
where
    T: TreeNodeType,
{
    pub node: &'a TreeNode<K, T>,
    pub batch: VecDeque<(K, V)>,
}

// Split the batch into smaller batch with associated node links
impl<'a, K: Key, V: Value> Iterator for Insert<'a, K, V, Branch> {
    type Item = (Cid, Vec<(K, V)>);

    fn next(&mut self) -> Option<Self::Item> {
        let mut kvs = Vec::new();
        let mut link = None;
        while let Some((batch_key, batch_value)) = self.batch.pop_front() {
            let idx = match self.node.keys.binary_search(&batch_key) {
                Ok(idx) => idx,
                Err(idx) => idx - 1, // Since links are ordered, the previous one has the correct range.
            };

            let Some(link) = link else {
                kvs.push((batch_key, batch_value));
                link = Some(self.node.values.links[idx]);
                continue;
            };

            if link != self.node.values.links[idx] {
                return Some((link, kvs));
            }

            kvs.push((batch_key, batch_value));
        }

        None
    }
}

pub struct Remove<'a, K, T>
where
    T: TreeNodeType,
{
    pub node: &'a mut TreeNode<K, T>,
    pub batch: VecDeque<K>,
}

// Split the batch into smaller batches with associated node links while removing batch keys
impl<'a, K: Key> Iterator for Remove<'a, K, Branch> {
    type Item = (Vec<Cid>, Vec<K>);

    fn next(&mut self) -> Option<Self::Item> {
        let mut range = None;
        let mut keys = Vec::new();
        let mut links = Vec::new();

        for i in 0..self.batch.len() {
            let batch_key = &self.batch[i];

            let (idx, remove) = match self.node.keys.binary_search(batch_key) {
                Ok(idx) => (idx, true),
                Err(idx) => (idx.saturating_sub(1), false),
            };

            if range.is_none() {
                range = Some((
                    Bound::Included(self.batch[i].clone()),
                    match self.node.keys.get(idx + 1) {
                        Some(key) => Bound::Excluded(key.clone()),
                        None => Bound::Unbounded,
                    },
                ));
            }

            if !range.as_ref().unwrap().contains(&batch_key) {
                return Some((links, keys));
            }

            if remove {
                self.batch.remove(i);

                let key = self.node.keys.remove(idx).unwrap();
                let link = self.node.values.links.remove(idx).unwrap();

                keys.push(key);
                links.push(link);
            } else {
                let batch_key = self.batch.pop_front().unwrap();

                keys.push(batch_key);
            }
        }

        None
    }
}

pub struct BranchIterator<'a, K> {
    pub node: &'a TreeNode<K, Branch>,
    pub index: usize,
}

impl<'a, K> Iterator for BranchIterator<'a, K> {
    type Item = ((Bound<&'a K>, Bound<&'a K>), &'a Cid);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.node.keys.len() {
            return None;
        }

        let key = &self.node.keys[self.index];
        let link = &self.node.values.links[self.index];

        let l_bound = Bound::Included(key);
        let h_bound = match self.node.keys.get(self.index + 1) {
            Some(key) => Bound::Excluded(key),
            None => Bound::Unbounded,
        };
        let range = (l_bound, h_bound);

        self.index += 1;

        Some((range, link))
    }
}

pub struct BranchIntoIterator<K> {
    pub node: TreeNode<K, Branch>,
}

impl<K: Key> Iterator for BranchIntoIterator<K> {
    type Item = ((Bound<K>, Bound<K>), Cid);

    fn next(&mut self) -> Option<Self::Item> {
        if self.node.keys.is_empty() {
            return None;
        }

        let key = self.node.keys.pop_front().unwrap();
        let link = self.node.values.links.pop_front().unwrap();

        let l_bound = Bound::Included(key);
        let h_bound = match self.node.keys.front() {
            Some(key) => Bound::Excluded(key.clone()),
            None => Bound::Unbounded,
        };
        let range = (l_bound, h_bound);

        Some((range, link))
    }
}

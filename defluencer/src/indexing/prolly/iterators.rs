use std::{collections::VecDeque, ops::Bound};

use super::tree::{Branch, Key, Leaf, TreeNode, TreeNodeType, Value};

use cid::Cid;

pub struct Search<'a, K, T>
where
    T: TreeNodeType,
{
    pub node: &'a TreeNode<K, T>,
    pub batch: VecDeque<K>,
    pub search_idx: usize,
}

// Split the batch into smaller batches with associated node links
impl<'a, K: Key> Iterator for Search<'a, K, Branch> {
    type Item = (Cid, Vec<K>);

    fn next(&mut self) -> Option<Self::Item> {
        let mut keys = Vec::new();
        let mut link = None;
        while let Some(batch_key) = self.batch.pop_front() {
            let idx = match self.node.keys[self.search_idx..].binary_search(&batch_key) {
                Ok(idx) => idx,
                Err(idx) => idx - 1, // Since links are ordered, the previous one has the correct range.
            };

            self.search_idx = idx;

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

        return None;
    }
}

// Find all the values associated with batch keys.
impl<'a, K: Key, V: Value> Iterator for Search<'a, K, Leaf<V>> {
    type Item = (K, V);

    // If we were to iter in reverse we could consume the node then swap remove instead of cloning

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(batch_key) = self.batch.pop_front() {
            if let Ok(idx) = self.node.keys[self.search_idx..].binary_search(&batch_key) {
                self.search_idx = idx;

                let value = self.node.values.elements[idx].clone();

                return Some((batch_key, value));
            }
        }

        return None;
    }
}

pub struct Insert<'a, K, V, T>
where
    T: TreeNodeType,
{
    pub node: &'a TreeNode<K, T>,
    pub batch: VecDeque<(K, V)>,
    pub search_idx: usize,
}

// Split the batch into smaller batch with associated node links
impl<'a, K: Key, V: Value> Iterator for Insert<'a, K, V, Branch> {
    type Item = (Cid, Vec<(K, V)>);

    fn next(&mut self) -> Option<Self::Item> {
        let mut kvs = Vec::new();
        let mut link = None;
        while let Some((batch_key, batch_value)) = self.batch.pop_front() {
            let idx = match self.node.keys[self.search_idx..].binary_search(&batch_key) {
                Ok(idx) => idx,
                Err(idx) => idx - 1, // Since links are ordered, the previous one has the correct range.
            };

            self.search_idx = idx;

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

        return None;
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
    pub node: TreeNode<K, Branch>,
    pub index: usize,
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

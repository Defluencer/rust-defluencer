use std::{
    collections::VecDeque,
    fmt::Debug,
    ops::{Bound, RangeBounds},
    vec,
};

use cid::Cid;

use serde::{Deserialize, Serialize};

use crate::indexing::ordered_trees::traits::{Key, Value};

use libipld_core::ipld::Ipld;

use super::iterators::{Insert, Remove, Search};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(bound = "K: Key, V: Value", try_from = "Ipld", into = "Ipld")]
pub struct TreeNode<K, V> {
    pub layer: usize,

    pub keys: VecDeque<K>,
    pub values: VecDeque<V>,

    /// Indexes at which inserting a link would preserve ordering.
    pub indexes: VecDeque<usize>,
    pub links: VecDeque<Cid>,
}

impl<K: Key, V: Value> TreeNode<K, V> {
    /// Insert sorted K-Vs into this node.
    ///
    /// Idempotent.
    pub fn batch_insert(&mut self, key_values: impl IntoIterator<Item = (K, V)>) {
        for (key, value) in key_values {
            match self.keys.binary_search(&key) {
                Ok(idx) => {
                    self.keys[idx] = key;
                    self.values[idx] = value;
                }
                Err(idx) => {
                    self.keys.insert(idx, key);
                    self.values.insert(idx, value);
                }
            }
        }
    }

    /// Remove matching keys from batch and node then merge batch ranges.
    pub fn batch_remove_match(&mut self, batch: &mut Batch<K, V>) {
        for j in (0..batch.elements.len()).rev() {
            let key = batch.elements[j].0.clone();

            for i in (0..self.keys.len()).rev() {
                let node_key = &self.keys[i];

                if *node_key == key {
                    self.keys.remove(i);
                    self.values.remove(i);
                    batch.elements.remove(j);

                    // Merge range before and after batch element
                    for k in 0..batch.ranges.len() - 1 {
                        let (l_low_b, l_up_b) = batch.ranges[k].clone();

                        if j == 0 && l_low_b == Bound::Excluded(key.clone()) {
                            batch.ranges[k].0 = Bound::Unbounded;
                            break;
                        }

                        let (r_low_b, r_up_b) = batch.ranges[k + 1].clone();

                        if l_up_b == Bound::Excluded(key.clone())
                            && r_low_b == Bound::Excluded(key.clone())
                        {
                            batch.ranges[k].1 = r_up_b;
                            batch.ranges.remove(k + 1);
                            break;
                        }

                        if j == (batch.elements.len() - 1) && r_up_b == Bound::Excluded(key.clone())
                        {
                            batch.ranges[k + 1].1 = Bound::Unbounded;
                            break;
                        }
                    }

                    break;
                }
            }
        }
    }

    /// Insert a link based on the range provided.
    ///
    /// Idempotent.
    pub fn insert_link(&mut self, link: Cid, range: (Bound<&K>, Bound<&K>)) {
        let start_idx = match range.start_bound() {
            Bound::Included(key) => match self.keys.binary_search(key) {
                Ok(_) => {
                    panic!("An included link range key should never be found in another node.")
                }
                Err(idx) => idx,
            },
            Bound::Excluded(key) => match self.keys.binary_search(key) {
                Ok(idx) => idx + 1,
                Err(idx) => idx,
            },
            Bound::Unbounded => 0,
        };

        let end_idx = match range.end_bound() {
            Bound::Included(key) => match self.keys.binary_search(key) {
                Ok(_) => {
                    panic!("An included link range key should never be found in another node.")
                }
                Err(idx) => idx,
            },
            Bound::Excluded(key) => match self.keys.binary_search(key) {
                Ok(idx) => idx,
                Err(idx) => idx,
            },
            Bound::Unbounded => self.keys.len(),
        };

        if start_idx != end_idx {
            panic!("Lower and higher bounds should always agree when inserting a link");
        }

        match self.indexes.binary_search(&start_idx) {
            Ok(idx) => {
                self.indexes[idx] = start_idx;
                self.links[idx] = link;
            }
            Err(idx) => {
                self.indexes.insert(idx, start_idx);
                self.links.insert(idx, link);
            }
        }
    }

    /// Remove all K-Vs and links outside of range.
    ///
    /// Idempotent.
    pub fn trim(&mut self, range: (Bound<&K>, Bound<&K>)) {
        let trunc_len = match range.end_bound() {
            Bound::Included(key) => match self.keys.binary_search(key) {
                Ok(idx) => Some(idx + 1),
                Err(idx) => Some(idx),
            },
            Bound::Excluded(key) => match self.keys.binary_search(key) {
                Ok(idx) => Some(idx),
                Err(idx) => Some(idx),
            },
            Bound::Unbounded => None,
        };

        if let Some(trunc_len) = trunc_len {
            self.keys.truncate(trunc_len);
            self.values.truncate(trunc_len);

            let last_idx = trunc_len - 1;
            let link_len = match self.indexes.binary_search(&last_idx) {
                Ok(idx) => idx + 1,
                Err(idx) => idx,
            };

            self.indexes.truncate(link_len);
            self.links.truncate(link_len);
        }

        let drain_idx = match range.start_bound() {
            Bound::Included(key) => match self.keys.binary_search(key) {
                Ok(idx) => Some(idx),
                Err(idx) => Some(idx),
            },
            Bound::Excluded(key) => match self.keys.binary_search(key) {
                Ok(idx) => Some(idx + 1),
                Err(idx) => Some(idx),
            },
            Bound::Unbounded => None,
        };

        if let Some(drain_idx) = drain_idx {
            self.keys.drain(..drain_idx);
            self.values.drain(..drain_idx);

            let link_idx = match self.indexes.binary_search(&drain_idx) {
                Ok(idx) => idx,
                Err(idx) => idx + 1,
            };

            self.indexes.drain(..link_idx);
            self.links.drain(..link_idx);
        }
    }

    /// Remove all elements. Returns keys, values and layers.
    pub fn rm_elements(&mut self) -> Vec<(K, V, usize)> {
        self.keys
            .drain(..)
            .zip(self.values.drain(..))
            .map(|(key, value)| (key, value, self.layer))
            .collect()
    }

    /// Remove all links and calculate each range bounds based on node keys.
    pub fn rm_link_ranges(&mut self) -> Vec<(Cid, (Bound<K>, Bound<K>))> {
        self.indexes
            .drain(..)
            .zip(self.links.drain(..))
            .map(|(idx, link)| {
                let low_b = {
                    if idx == 0 {
                        Bound::Unbounded
                    } else {
                        match self.keys.get(idx - 1) {
                            Some(key) => Bound::Excluded(key.clone()),
                            None => Bound::Unbounded,
                        }
                    }
                };

                let up_b = match self.keys.get(idx) {
                    Some(key) => Bound::Excluded(key.clone()),
                    None => Bound::Unbounded,
                };

                let range = (low_b, up_b);

                (link, range)
            })
            .collect()
    }

    /// Returns node elements and each link with range.
    pub fn into_inner(mut self) -> (Vec<(K, V, usize)>, Vec<(Cid, (Bound<K>, Bound<K>))>) {
        let link_ranges = self.rm_link_ranges();
        let elements = self.rm_elements();

        (elements, link_ranges)
    }

    /// Merge all elements and links of two nodes.
    pub fn merge(&mut self, other: Self) {
        if self.layer != other.layer {
            panic!("Can never merge node with different layer");
        }

        let (elements, link_ranges) = other.into_inner();

        self.batch_insert(elements.into_iter().map(|(key, value, _)| (key, value)));

        for (link, (lb, hb)) in link_ranges {
            self.insert_link(link, (lb.as_ref(), hb.as_ref()));
        }
    }

    /// Return either splitted batches or the KVs searched for.
    pub fn into_search_iter(self, batch: impl IntoIterator<Item = K>) -> Search<K, V> {
        Search {
            node: self,
            batch: batch.into_iter().collect(),
            offset: 0,
        }
    }

    /// Adds the batch KVs in this node, update links and
    /// returns; links, ranges and splitted batches.
    pub fn insert_iter(&mut self, batch: impl IntoIterator<Item = (K, V, usize)>) -> Insert<K, V> {
        Insert {
            node: self,
            batch: batch.into_iter().collect(),
            outdated_link_idx: Vec::new(),
        }
    }

    /// Remove the batch keys in this node, update links and
    /// returns; links, ranges and splitted batches.
    pub fn remove_iter(
        &mut self,
        node_range: (Bound<K>, Bound<K>),
        batch: impl IntoIterator<Item = K>,
    ) -> Remove<K, V> {
        Remove {
            node: self,
            batch: batch.into_iter().collect(),
            node_range,
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct Batch<K, V> {
    pub elements: VecDeque<(K, V, usize)>,      // key, value, layer
    pub ranges: VecDeque<(Bound<K>, Bound<K>)>, // lower bound, upper bound
}

impl<K: Key, V: Value> Batch<K, V> {
    /// Insert sorted elements into this batch.
    ///
    /// Idempotent.
    pub fn batch_insert(
        &mut self,
        iter: impl IntoIterator<Item = (K, V, usize)>
            + Iterator<Item = (K, V, usize)>
            + DoubleEndedIterator,
    ) {
        let mut stop = self.elements.len();
        for (key, value, layer) in iter.rev() {
            for i in (0..stop).rev() {
                let batch_key = self.elements[i].0.clone();

                if batch_key < key {
                    self.elements.insert(i + 1, (key, value, layer));
                    stop = i + 1;
                    break;
                }

                if batch_key == key {
                    self.elements[i] = (key, value, layer);
                    stop = i;
                    break;
                }
            }
        }
    }

    /// Remove all element at highest layer and split the ranges.
    pub fn rm_highest(&mut self) -> (VecDeque<K>, VecDeque<V>, usize) {
        let highest_l = self
            .elements
            .iter()
            .fold(0, |state, (_, _, layer)| state.max(*layer));

        let mut rm_keys = VecDeque::with_capacity(self.elements.len());
        let mut rm_values = VecDeque::with_capacity(self.elements.len());

        self.elements.retain(|(key, value, layer)| {
            let pred = *layer != highest_l;

            if !pred {
                for i in 0..self.ranges.len() {
                    let range = self.ranges[i].clone();

                    if range.contains(key) {
                        let old_up_b = range.1;

                        self.ranges[i].1 = Bound::Excluded(key.clone());
                        let new_low_b = Bound::Excluded(key.clone());

                        let new_up_b = old_up_b;

                        // Empty range are not fine
                        if new_low_b != new_up_b {
                            let new_range = (new_low_b, new_up_b);

                            self.ranges.insert(i + 1, new_range);
                        }
                    }
                }

                rm_keys.push_back(key.clone());
                rm_values.push_back(value.clone());
            }

            pred
        });

        (rm_keys, rm_values, highest_l)
    }

    /// Split a multi-range batch into multiple single range batch.
    pub fn split_per_range(mut self) -> Vec<Self> {
        if self.ranges.len() < 2 {
            return Vec::default();
        }

        let mut batches = Vec::with_capacity(self.ranges.len());

        for range in self.ranges.into_iter() {
            let mut elements = VecDeque::with_capacity(self.elements.len());
            self.elements.retain(|(key, value, layer)| {
                let pred = !range.contains(key);

                if !pred {
                    elements.push_back((key.clone(), value.clone(), *layer));
                }

                pred
            });

            let batch = Self {
                elements,
                ranges: VecDeque::from(vec![range]),
            };

            batches.push(batch);
        }

        batches
    }
}

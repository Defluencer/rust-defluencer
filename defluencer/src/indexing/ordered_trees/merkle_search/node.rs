use std::{
    collections::VecDeque,
    fmt::Debug,
    ops::{Bound, RangeBounds},
};

use cid::Cid;

use serde::{Deserialize, Serialize};

use crate::indexing::ordered_trees::traits::{Key, Value};

use libipld_core::ipld::Ipld;

use super::iterators::{Insert, Remove, Search};

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(bound = "K: Key, V: Value", try_from = "Ipld", into = "Ipld")]
pub struct TreeNode<K, V> {
    pub layer: usize,

    pub keys: VecDeque<K>,
    pub values: VecDeque<V>,

    /// Indices at which inserting a link would preserve ordering.
    pub indices: VecDeque<usize>,
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

        match self.indices.binary_search(&start_idx) {
            Ok(idx) => {
                self.indices[idx] = start_idx;
                self.links[idx] = link;
            }
            Err(idx) => {
                self.indices.insert(idx, start_idx);
                self.links.insert(idx, link);
            }
        }
    }

    /// Remove all K-Vs and links outside of range.
    ///
    /// Idempotent.
    pub fn crop(&mut self, range: (Bound<&K>, Bound<&K>)) {
        let trunc = match range.end_bound() {
            Bound::Included(key) => match self.keys.binary_search(key) {
                Ok(idx) => Some((idx + 1, false)),
                Err(idx) => Some((idx, false)),
            },
            Bound::Excluded(key) => match self.keys.binary_search(key) {
                Ok(idx) => Some((idx, true)),
                Err(idx) => Some((idx, false)),
            },
            Bound::Unbounded => None,
        };

        if let Some((length, extra)) = trunc {
            if length < self.keys.len() {
                self.keys.truncate(length);
                self.values.truncate(length);

                let link_idx = length + 1;
                let mut link_len = match self.indices.binary_search(&link_idx) {
                    Ok(idx) => idx - 1,
                    Err(idx) => idx,
                };

                if extra {
                    link_len += 1;
                }

                self.indices.truncate(link_len);
                self.links.truncate(link_len);
            }
        }

        let drain = match range.start_bound() {
            Bound::Included(key) => match self.keys.binary_search(key) {
                Ok(idx) => Some((idx, false)),
                Err(idx) => Some((idx, false)),
            },
            Bound::Excluded(key) => match self.keys.binary_search(key) {
                Ok(idx) => Some((idx + 1, true)),
                Err(idx) => Some((idx, false)),
            },
            Bound::Unbounded => None,
        };

        if let Some((idx, extra)) = drain {
            if idx != 0 {
                self.keys.drain(..idx);
                self.values.drain(..idx);

                let mut link_idx = match self.indices.binary_search(&idx) {
                    Ok(idx) => idx + 1,
                    Err(idx) => idx + 2,
                };

                let offset = link_idx - 1;

                if extra {
                    link_idx -= 1;
                }

                self.indices.drain(..link_idx);
                self.links.drain(..link_idx);

                self.indices.iter_mut().for_each(|i| *i -= offset);
            }
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
        self.indices
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
            panic!("Can never merge nodes with different layer");
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

    /// Adds the batch KVs in this node, remove outdated links and
    /// iterate through links, ranges and  batches.
    pub fn insert_iter(&mut self, batch: impl IntoIterator<Item = (K, V, usize)>) -> Insert<K, V> {
        Insert {
            node: self,
            batch: batch.into_iter().collect(),
            outdated_link_idx: Vec::new(),
            split_second_half: None,
        }
    }

    /// Remove the batch keys in this node, remove outdated links and
    /// iterate through links, ranges and batches.
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

#[cfg(test)]
mod tests {
    use super::*;

    use rand_core::RngCore;

    use rand_xoshiro::{rand_core::SeedableRng, Xoshiro256StarStar};

    #[test]
    fn node_insert_link() {
        let keys = VecDeque::from(vec![1, 3, 5, 7, 9, 11]);
        let indices = VecDeque::from(vec![1, 3, 4, 5]);

        let mut rng = Xoshiro256StarStar::from_entropy();

        let values: VecDeque<_> = (0..keys.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();
        let links: VecDeque<_> = (0..indices.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();

        let mut node = TreeNode {
            layer: 0,
            keys: keys.clone(),
            values,
            indices,
            links,
        };

        let link = random_cid(&mut rng);
        let range = (Bound::Excluded(&keys[1]), Bound::Excluded(&keys[2]));

        node.insert_link(link, range);

        assert_eq!(node.links.len(), 5);
        assert_eq!(node.links[1], link);

        let link = random_cid(&mut rng);
        let range = (Bound::Unbounded, Bound::Excluded(&keys[0]));

        node.insert_link(link, range);

        assert_eq!(node.links.len(), 6);
        assert_eq!(node.links[0], link);

        let link = random_cid(&mut rng);
        let range = (Bound::Excluded(&keys[5]), Bound::Unbounded);

        node.insert_link(link, range);

        assert_eq!(node.links.len(), 7);
        assert_eq!(node.links[6], link);
    }

    #[test]
    fn node_crop() {
        let keys = VecDeque::from(vec![1, 3, 5, 7, 9, 11]);
        let indices = VecDeque::from(vec![0, 1, 2, 3, 4, 5, 6]);

        let mut rng = Xoshiro256StarStar::from_entropy();

        let values: VecDeque<_> = (0..keys.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();
        let links: VecDeque<_> = (0..indices.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();

        let og_node = TreeNode {
            layer: 0,
            keys: keys.clone(),
            values,
            indices: indices.clone(),
            links: links.clone(),
        };

        //remove first key link
        let mut node = og_node.clone();
        let range = (Bound::Excluded(&1), Bound::Unbounded);
        node.crop(range);
        node.crop(range);
        assert_eq!(node.keys, vec![3, 5, 7, 9, 11]);
        assert_eq!(node.indices, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(node.links[0], links[1]);

        //remove first key and first 2 links
        let mut node = og_node.clone();
        let range = (Bound::Included(&3), Bound::Unbounded);
        node.crop(range);
        node.crop(range);
        assert_eq!(node.keys, vec![3, 5, 7, 9, 11]);
        assert_eq!(node.indices, vec![1, 2, 3, 4, 5]);
        assert_eq!(node.links[0], links[2]);

        //remove last key and link
        let mut node = og_node.clone();
        let range = (Bound::Unbounded, Bound::Excluded(&11));
        node.crop(range);
        node.crop(range);
        assert_eq!(node.keys, vec![1, 3, 5, 7, 9]);
        assert_eq!(node.indices, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(node.links[5], links[5]);

        //remove last key and last 2 links
        let mut node = og_node.clone();
        let range = (Bound::Unbounded, Bound::Included(&9));
        node.crop(range);
        node.crop(range);
        assert_eq!(node.keys, vec![1, 3, 5, 7, 9]);
        assert_eq!(node.indices, vec![0, 1, 2, 3, 4]);
        assert_eq!(node.links[4], links[4]);

        //remove nothing
        let mut node = og_node.clone();
        let range = (Bound::Excluded(&0), Bound::Excluded(&12));
        node.crop(range);
        node.crop(range);
        assert_eq!(node.keys, keys);
        assert_eq!(node.indices, indices);
    }

    fn random_cid(rng: &mut Xoshiro256StarStar) -> Cid {
        let mut input = [0u8; 64];
        rng.fill_bytes(&mut input);

        use sha2::Digest;
        let hash = sha2::Sha512::new_with_prefix(input).finalize();

        let multihash = multihash::Multihash::wrap(0x13, &hash).unwrap();

        Cid::new_v1(/* DAG-CBOR */ 0x71, multihash)
    }
}

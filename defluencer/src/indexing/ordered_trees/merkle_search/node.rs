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
        /* #[cfg(debug_assertions)]
        println!("Insert Link\nIn Keys {:?}\nAt Range {:?}", self.keys, range); */

        let (start_idx, end_idx) = match (range.start_bound(), range.end_bound()) {
            (Bound::Excluded(start_key), Bound::Excluded(end_key)) => {
                let start_idx = match self.keys.binary_search(start_key) {
                    Ok(idx) => idx + 1,
                    Err(idx) => idx,
                };

                let end_idx = self.keys.binary_search(end_key).unwrap_or_else(|x| x);

                (start_idx, end_idx)
            }
            (Bound::Unbounded, Bound::Excluded(end_key)) => {
                let end_idx = match self.keys.binary_search(end_key) {
                    Ok(idx) => idx,
                    Err(idx) => idx,
                };

                (0, end_idx)
            }
            (Bound::Excluded(start_key), Bound::Unbounded) => {
                let start_idx = match self.keys.binary_search(start_key) {
                    Ok(idx) => idx + 1,
                    Err(idx) => idx,
                };

                (start_idx, self.keys.len())
            }
            (Bound::Unbounded, Bound::Unbounded) => (0, self.keys.len()),
            _ => panic!("never used"),
        };

        if start_idx != end_idx {
            let idx = self.indices.binary_search(&end_idx).unwrap_or_else(|x| x);

            self.indices.insert(idx, end_idx);
            self.links.insert(idx, link);

            return;
        }

        match self.indices.binary_search(&end_idx) {
            Ok(idx) => {
                self.indices[idx] = end_idx;
                self.links[idx] = link;
            }
            Err(idx) => {
                self.indices.insert(idx, end_idx);
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
    pub fn rm_link_ranges(
        &mut self,
        node_range: &(Bound<K>, Bound<K>),
    ) -> Vec<(Cid, (Bound<K>, Bound<K>))> {
        self.indices
            .drain(..)
            .zip(self.links.drain(..))
            .map(|(idx, link)| {
                let low_b = {
                    if idx == 0 {
                        node_range.start_bound().cloned()
                    } else {
                        match self.keys.get(idx - 1) {
                            Some(key) => Bound::Excluded(key.clone()),
                            None => node_range.start_bound().cloned(),
                        }
                    }
                };
                let up_b = match self.keys.get(idx) {
                    Some(key) => Bound::Excluded(key.clone()),
                    None => node_range.end_bound().cloned(),
                };
                let range = (low_b, up_b);

                (link, range)
            })
            .collect()
    }

    /// Returns node elements and each link with range.
    pub fn into_inner(
        mut self,
        node_range: &(Bound<K>, Bound<K>),
    ) -> (Vec<(K, V, usize)>, Vec<(Cid, (Bound<K>, Bound<K>))>) {
        let link_ranges = self.rm_link_ranges(node_range);
        let elements = self.rm_elements();

        (elements, link_ranges)
    }

    /// Merge all elements and links of two nodes.
    pub fn merge(
        &mut self,
        node_range: &(Bound<K>, Bound<K>),
        mut other: Self,
        other_range: &(Bound<K>, Bound<K>),
    ) {
        /* #[cfg(debug_assertions)]
        println!(
            "Merging Nodes\nKeys\n{:?}\n{:?}\nIndices\n{:?}\n{:?}",
            self.keys, other.keys, self.indices, other.indices
        ); */

        if self.layer != other.layer {
            panic!("Cannot Merge Nodes With Different Layers!");
        }

        if other.keys.is_empty() && other.links.len() == 1 {
            self.indices.push_back(self.keys.len());
            self.links.push_back(other.links[0]);

            /* #[cfg(debug_assertions)]
            println!(
                "Merged Node\nKeys\n{:?}\nIndices\n{:?}",
                self.keys, self.indices,
            ); */

            return;
        }

        if self.keys.is_empty() && self.links.len() == 1 {
            self.keys = other.keys;
            self.values = other.values;

            self.indices.extend(other.indices);
            self.links.extend(other.links);

            /* #[cfg(debug_assertions)]
            println!(
                "Merged Node\nKeys\n{:?}\nIndices\n{:?}",
                self.keys, self.indices,
            ); */

            return;
        }

        if self.keys.front() < other.keys.front() {
            let link_ranges = other.rm_link_ranges(other_range);

            self.keys.extend(other.keys);
            self.values.extend(other.values);

            for (link, range) in link_ranges {
                self.insert_link(link, (range.start_bound(), range.end_bound()));
            }
        } else {
            let link_ranges = self.rm_link_ranges(node_range);
            let elements = self.rm_elements();

            other.batch_insert(elements.into_iter().map(|(key, value, _)| (key, value)));

            for (link, range) in link_ranges {
                other.insert_link(link, (range.start_bound(), range.end_bound()));
            }

            *self = other;
        }

        /* #[cfg(debug_assertions)]
        println!(
            "Merged Node\nKeys\n{:?}\nIndices\n{:?}",
            self.keys, self.indices,
        ); */
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
    pub fn insert_iter<'a>(
        &'a mut self,
        node_range: &'a (Bound<K>, Bound<K>),
        batch: impl IntoIterator<Item = (K, V, usize)>,
        link_ranges: impl IntoIterator<Item = (Cid, (Bound<K>, Bound<K>))>,
    ) -> Insert<K, V> {
        let batch: VecDeque<_> = batch.into_iter().collect();

        self.layer = batch
            .iter()
            .fold(0, |state, (_, _, layer)| state.max(*layer));

        Insert {
            node: self,
            node_range,
            batch,
            link_ranges: link_ranges.into_iter().collect(),
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
            key_rm_idx: vec![],
            indices_rm_idx: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rand::Rng;
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

    #[test]
    fn node_merge() {
        let mut rng = Xoshiro256StarStar::from_entropy();

        for _ in 0..100 {
            let mut kvs = Vec::with_capacity(20);
            let mut index_links: Vec<(usize, Cid)> = Vec::with_capacity(10);

            for _ in 0..20 {
                let key = rng.next_u32() as usize;
                let value = random_cid(&mut rng);

                kvs.push((key, value));
            }

            kvs.sort_unstable_by(|(key, _), (other, _)| key.cmp(other));
            kvs.dedup_by(|(key, _), (other, _)| key == other);

            for _ in 0..10 {
                let mut index = rng.gen_range(0..kvs.len() + 1);

                while index_links.binary_search_by(|(k, _)| k.cmp(&index)).is_ok() {
                    index = rng.gen_range(0..kvs.len());
                }

                let link = random_cid(&mut rng);

                index_links.push((index, link));
            }

            index_links.sort_unstable_by(|(idx, _), (other, _)| idx.cmp(other));
            index_links.dedup_by(|(idx, _), (other, _)| idx == other);
            let part_i = index_links.partition_point(|&(idx, _)| idx < 11);
            //println!("Partition {}", part_i);

            let second_kvs = kvs.clone().split_off(10);
            let (keys, values) = second_kvs.into_iter().unzip();

            let second_index_links = index_links.clone().split_off(part_i);
            let (mut indices, links): (VecDeque<_>, _) = second_index_links.into_iter().unzip();

            indices.iter_mut().for_each(|idx| *idx -= 10);

            let node_two = TreeNode {
                layer: 0,
                keys,
                values,
                indices,
                links,
            };

            let mut first_kvs = kvs.clone();
            first_kvs.truncate(10);
            let (keys, values) = first_kvs.into_iter().unzip();

            let mut first_index_links = index_links.clone();
            first_index_links.truncate(part_i);
            let (indices, links) = first_index_links.into_iter().unzip();

            let mut node_one = TreeNode {
                layer: 0,
                keys,
                values,
                indices,
                links,
            };

            /* println!(
                "Node One Keys {:?} Indices {:?}",
                node_one.keys, node_one.indices
            ); */

            /* println!(
                "Node Two Keys {:?} Indices {:?}",
                node_two.keys, node_two.indices
            ); */

            node_one.merge(
                &(Bound::Unbounded, Bound::Unbounded),
                node_two,
                &(Bound::Unbounded, Bound::Unbounded),
            );
            let result = node_one;

            let (keys, values) = kvs.into_iter().unzip();
            let (indices, links) = index_links.into_iter().unzip();

            let expected = TreeNode {
                layer: 0,
                keys,
                values,
                indices,
                links,
            };

            assert_eq!(result, expected);
        }
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

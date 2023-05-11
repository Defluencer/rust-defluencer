use std::{
    collections::VecDeque,
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
        if self.node.indices.front() == Some(&self.index) {
            let link_idx = self.node.indices.pop_front().unwrap();

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

                    if let Ok(idx) = self.node.indices.binary_search(&idx) {
                        // Found matching link

                        match link {
                            Some(link) => {
                                if link != self.node.links[idx] {
                                    return Some(Either::Left((link, keys)));
                                }
                            }
                            None => {
                                link = Some(self.node.links[idx]);
                            }
                        }

                        let batch_key = self.batch.pop_front().unwrap();
                        keys.push(batch_key);
                        continue;
                    }

                    // No link found
                    self.batch.pop_front().unwrap();
                }
            }
        }

        if link.is_some() && !keys.is_empty() {
            return Some(Either::Left((link.unwrap(), keys)));
        }

        None
    }
}

pub struct Insert<'a, K, V> {
    pub node: &'a mut TreeNode<K, V>,
    pub batch: VecDeque<(K, V, usize)>,
    pub outdated_link_idx: Vec<usize>,
    pub split_second_half: Option<(Cid, (Bound<K>, Bound<K>), Vec<(K, V, usize)>)>,
}

impl<'a, K: Key, V: Value> Iterator for Insert<'a, K, V> {
    type Item = (Option<Cid>, (Bound<K>, Bound<K>), Vec<(K, V, usize)>);

    fn next(&mut self) -> Option<Self::Item> {
        let mut batch = Vec::new();
        let mut link = None;
        while let Some((batch_key, _, batch_layer)) = self.batch.front() {
            let (key_idx, key_found) = match self.node.keys.binary_search(batch_key) {
                Ok(idx) => (idx, true),
                Err(idx) => (idx, false),
            };

            if *batch_layer != self.node.layer {
                let (link_idx, link_found) = match self.node.indices.binary_search(&key_idx) {
                    Ok(i) => (i, true),
                    Err(i) => (i, false),
                };

                if self.split_second_half.is_some() {
                    match (
                        self.split_second_half.as_mut(),
                        self.node.links.get(link_idx),
                    ) {
                        (Some(split_node), Some(link)) if *link == split_node.0 => {
                            let item = self.batch.pop_front().unwrap();
                            split_node.2.push(item);
                            continue;
                        }
                        _ => {}
                    }

                    let node = self.split_second_half.take().unwrap();

                    return Some((Some(node.0), node.1, node.2));
                }

                if link_found && link.is_none() {
                    self.outdated_link_idx.push(link_idx);

                    let cid = self.node.links[link_idx];
                    link = Some(cid)
                }

                let item = self.batch.pop_front().unwrap();
                batch.push(item);
                continue;
            }

            if self.split_second_half.is_some() {
                let node = self.split_second_half.take().unwrap();

                return Some((Some(node.0), node.1, node.2));
            }

            if link.is_none() && batch.len() > 0 {
                let (batch_key, _, _) = batch.first().unwrap();

                let key_idx = match self.node.keys.binary_search(batch_key) {
                    Ok(i) => i,
                    Err(i) => i.saturating_sub(1),
                };

                let lb = if key_idx == 0 {
                    Bound::Unbounded
                } else {
                    Bound::Excluded(self.node.keys[key_idx].clone())
                };
                let hb = Bound::Excluded(self.node.keys[key_idx + 1].clone());
                let range = (lb, hb);

                return Some((link, range, batch));
            }

            let (key, value, _) = self.batch.pop_front().unwrap();

            if key_found {
                self.node.values[key_idx] = value;
                continue;
            }

            self.node.keys.insert(key_idx, key.clone());
            self.node.values.insert(key_idx, value);

            let (link_idx, split) = match self.node.indices.binary_search(&key_idx) {
                Ok(i) => (i, true),
                Err(i) => (i, false),
            };

            // Shift all the indexes after this add
            self.node
                .indices
                .iter_mut()
                .skip(link_idx)
                .for_each(|index| *index += 1);

            if split {
                let lb = if key_idx == 0 {
                    Bound::Unbounded
                } else {
                    Bound::Excluded(self.node.keys[key_idx - 1].clone())
                };
                let hb = Bound::Excluded(key);
                let range = (lb, hb);

                if self.outdated_link_idx.last() != Some(&link_idx) {
                    self.outdated_link_idx.push(link_idx);
                }

                let split_link = self.node.links[link_idx];

                let split_first_half = (Some(split_link), range, batch);

                let lb = Bound::Excluded(self.node.keys[key_idx].clone());
                let hb = match self.node.keys.get(key_idx + 1) {
                    Some(key) => Bound::Excluded(key.clone()),
                    None => Bound::Unbounded,
                };
                let range = (lb, hb);

                self.split_second_half = Some((split_link, range, vec![]));

                return Some(split_first_half);
            }
        }

        if !batch.is_empty() {
            let (batch_key, _, _) = batch.first().unwrap();
            let key_idx = self
                .node
                .keys
                .binary_search(batch_key)
                .unwrap_or_else(|x| x);

            let lb = if key_idx == 0 {
                Bound::Unbounded
            } else {
                Bound::Excluded(self.node.keys[key_idx - 1].clone())
            };
            let hb = if key_idx == self.node.keys.len() {
                Bound::Unbounded
            } else {
                Bound::Excluded(self.node.keys[key_idx].clone())
            };
            let range = (lb, hb);

            return Some((link, range, batch));
        }

        if let Some(node) = self.split_second_half.take() {
            return Some((Some(node.0), node.1, node.2));
        }

        for idx in self.outdated_link_idx.drain(..).rev() {
            self.node.indices.remove(idx).unwrap();
            self.node.links.remove(idx).unwrap();
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
    type Item = (Vec<Cid>, (Bound<K>, Bound<K>), Vec<K>);

    fn next(&mut self) -> Option<Self::Item> {
        let mut key_index = None;
        let mut range: Option<(Bound<K>, Bound<K>)> = None;
        let mut links = Vec::new();
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
                    .indices
                    .binary_search(&key_idx)
                    .unwrap_or_else(|x| x);

                // Shift all the link indexes after this remove
                self.node
                    .indices
                    .iter_mut()
                    .skip(link_idx)
                    .for_each(|index| *index -= 1);

                continue;
            }

            let link_idx = match self.node.indices.binary_search(&key_idx) {
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

            // Since we shift all key index in link indices
            // some will have the same key index
            // that means we need to add those nodes to the same batch for merging

            if key_index != Some(key_idx) && range.is_some() {
                // Previous batch is not the same
                let mut range = range.unwrap();

                range.1 = Bound::Excluded(self.node.keys[key_idx - 1].clone());

                return Some((links, range, batch));
            }

            key_index = Some(key_idx);

            let batch_key = self.batch.pop_front().unwrap();

            self.node.indices.remove(link_idx).unwrap();
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
            links.push(cid);
            batch.push(batch_key);
        }

        if !batch.is_empty() {
            let mut range = range.unwrap();

            let key_idx = self
                .node
                .keys
                .binary_search(&batch.last().unwrap())
                .unwrap_or_else(|x| x);

            range.1 = if key_idx == self.node.keys.len() {
                self.node_range.end_bound().cloned()
            } else {
                Bound::Excluded(self.node.keys[key_idx].clone())
            };

            return Some((links, range, batch));
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rand_core::RngCore;

    use rand_xoshiro::{rand_core::SeedableRng, Xoshiro256StarStar};

    use sha2::{Digest, Sha512};

    use cid::Cid;

    use multihash::Multihash;

    #[test]
    fn search_iter() {
        let keys = VecDeque::from(vec![2, 3, 5, 7, 9, 10]);
        let indices = VecDeque::from(vec![0, 3, 6]);

        let batch = VecDeque::from(vec![0, 1, 3, 5, 6, 8, 9, 11, 12]);

        let mut rng = Xoshiro256StarStar::from_entropy();

        let values: VecDeque<_> = (0..keys.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();
        let links: VecDeque<_> = (0..indices.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();

        let node = TreeNode::<u8, Cid> {
            layer: 1,
            keys: keys.clone(),
            values: values.clone(),
            indices: indices.clone(),
            links: links.clone(),
        };

        let mut expected_results = Vec::with_capacity(7);

        //batch key 0 & 1
        let expected_link = links[0];
        let expected_keys = vec![batch[0], batch[1]];
        let new_batch = Either::Left((expected_link, expected_keys));
        expected_results.push(new_batch);

        //batch key 3
        let expected_key = keys[1];
        let expected_value = values[1];
        let new_batch = Either::Right((expected_key, expected_value));
        expected_results.push(new_batch);

        //batch key 5
        let expected_key = keys[2];
        let expected_value = values[2];
        let new_batch = Either::Right((expected_key, expected_value));
        expected_results.push(new_batch);

        //batch key 6
        let expected_link = links[1];
        let expected_keys = vec![batch[4]];
        let new_batch = Either::Left((expected_link, expected_keys));
        expected_results.push(new_batch);

        //batch key 8 is not found

        //batch key 9
        let expected_key = keys[4];
        let expected_value = values[4];
        let new_batch = Either::Right((expected_key, expected_value));
        expected_results.push(new_batch);

        //batch key 11 & 12
        let expected_link = links[2];
        let expected_keys = vec![batch[7], batch[8]];
        let new_batch = Either::Left((expected_link, expected_keys));
        expected_results.push(new_batch);

        let iter = Search {
            node,
            batch,
            offset: 0,
        };

        let results: Vec<_> = iter.collect();

        assert_eq!(results, expected_results);
    }

    #[test]
    fn insert_iter_split_first_link() {
        let keys = VecDeque::from(vec![50]);
        let indices = VecDeque::from(vec![0]);

        let batch = vec![
            (10, 0), // Add KV to split first half
            (20, 1), // Split first link
            (40, 0), // Add KV to split later half
            (41, 0), // Add KV to split later half
        ];

        let mut rng = Xoshiro256StarStar::from_entropy();

        let values: VecDeque<_> = (0..keys.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();
        let links: VecDeque<_> = (0..indices.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();

        let mut node = TreeNode::<u16, Cid> {
            layer: 1,
            keys: keys.clone(),
            values: values.clone(),
            indices: indices.clone(),
            links: links.clone(),
        };
        let node = &mut node;

        let batch: VecDeque<_> = batch
            .into_iter()
            .map(|(key, lvl)| (key, random_cid(&mut rng), lvl))
            .collect();

        let mut expected_results = Vec::with_capacity(2);

        //batch key 10
        let expected_link = Some(links[0]);
        let expected_range = (Bound::Unbounded, Bound::Excluded(20));
        let expected_batch = vec![batch[0]];
        let new_batch = (expected_link, expected_range, expected_batch);
        expected_results.push(new_batch);

        //batch key 40, 41
        let expected_link = Some(links[0]);
        let expected_range = (Bound::Excluded(20), Bound::Excluded(50));
        let expected_batch = vec![batch[2], batch[3]];
        let new_batch = (expected_link, expected_range, expected_batch);
        expected_results.push(new_batch);

        let iter = Insert {
            node,
            batch,
            outdated_link_idx: vec![],
            split_second_half: None,
        };

        let results: Vec<_> = iter.collect();

        assert_eq!(results, expected_results);
        assert_eq!(node.keys[0], 20);
        assert!(node.links.is_empty());
    }

    #[test]
    fn insert_iter_split_middle_link() {
        let keys = VecDeque::from(vec![50, 220, 280, 300, 400]);
        let indices = VecDeque::from(vec![2]);

        let batch = vec![
            (230, 0), // Add KV to split first half
            (250, 1), // Split link
            (270, 0), // Add KV to split later half
            (271, 0), // Add KV to split later half
        ];

        let mut rng = Xoshiro256StarStar::from_entropy();

        let values: VecDeque<_> = (0..keys.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();
        let links: VecDeque<_> = (0..indices.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();

        let mut node = TreeNode::<u16, Cid> {
            layer: 1,
            keys: keys.clone(),
            values: values.clone(),
            indices: indices.clone(),
            links: links.clone(),
        };
        let node = &mut node;

        let batch: VecDeque<_> = batch
            .into_iter()
            .map(|(key, lvl)| (key, random_cid(&mut rng), lvl))
            .collect();

        let mut expected_results = Vec::with_capacity(2);

        //batch key 230
        let expected_link = Some(links[0]);
        let expected_range = (Bound::Excluded(220), Bound::Excluded(250));
        let expected_batch = vec![batch[0]];
        let new_batch = (expected_link, expected_range, expected_batch);
        expected_results.push(new_batch);

        //batch key 270, 271
        let expected_link = Some(links[0]);
        let expected_range = (Bound::Excluded(250), Bound::Excluded(280));
        let expected_batch = vec![batch[2], batch[3]];
        let new_batch = (expected_link, expected_range, expected_batch);
        expected_results.push(new_batch);

        let iter = Insert {
            node,
            batch,
            outdated_link_idx: vec![],
            split_second_half: None,
        };

        let results: Vec<_> = iter.collect();

        assert_eq!(results, expected_results);
        assert_eq!(node.keys[2], 250);
        assert!(node.links.is_empty());
    }

    #[test]
    fn insert_iter_split_link_end() {
        let keys = VecDeque::from(vec![50, 220, 280, 300, 400]);
        let indices = VecDeque::from(vec![0, 2, 5]);

        let batch = vec![
            (418, 0), // Add KV to split first half
            (420, 1), // Split last link
            (421, 0), // Add KV to split later half
            (422, 0), // Add KV to split later half
        ];

        let mut rng = Xoshiro256StarStar::from_entropy();

        let values: VecDeque<_> = (0..keys.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();
        let links: VecDeque<_> = (0..indices.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();

        let mut node = TreeNode::<u16, Cid> {
            layer: 1,
            keys: keys.clone(),
            values: values.clone(),
            indices: indices.clone(),
            links: links.clone(),
        };
        let node = &mut node;

        let batch: VecDeque<_> = batch
            .into_iter()
            .map(|(key, lvl)| (key, random_cid(&mut rng), lvl))
            .collect();

        let mut expected_results = Vec::with_capacity(2);

        //batch key 418
        let expected_link = Some(links[2]);
        let expected_range = (Bound::Excluded(400), Bound::Excluded(420));
        let expected_batch = vec![batch[0]];
        let new_batch = (expected_link, expected_range, expected_batch);
        expected_results.push(new_batch);

        //batch key 421, 422
        let expected_link = Some(links[2]);
        let expected_range = (Bound::Excluded(420), Bound::Unbounded);
        let expected_batch = vec![batch[2], batch[3]];
        let new_batch = (expected_link, expected_range, expected_batch);
        expected_results.push(new_batch);

        let iter = Insert {
            node,
            batch,
            outdated_link_idx: vec![],
            split_second_half: None,
        };

        let results: Vec<_> = iter.collect();

        assert_eq!(results, expected_results);
        assert_eq!(node.keys[5], 420);
        assert_eq!(node.links.len(), 2);
    }

    #[test]
    fn insert_iter_add_link_start() {
        let keys = VecDeque::from(vec![50, 220, 280, 300, 400]);
        let indices = VecDeque::from(vec![1]);

        let batch = vec![
            (10, 0), // Add new link
            (11, 0), // Add new link
        ];

        let mut rng = Xoshiro256StarStar::from_entropy();

        let values: VecDeque<_> = (0..keys.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();
        let links: VecDeque<_> = (0..indices.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();

        let mut node = TreeNode::<u16, Cid> {
            layer: 1,
            keys: keys.clone(),
            values: values.clone(),
            indices: indices.clone(),
            links: links.clone(),
        };
        let node = &mut node;

        let batch: VecDeque<_> = batch
            .into_iter()
            .map(|(key, lvl)| (key, random_cid(&mut rng), lvl))
            .collect();

        let mut expected_results = Vec::with_capacity(1);

        //batch key 10, 11
        let expected_link = None;
        let expected_range = (Bound::Unbounded, Bound::Excluded(50));
        let expected_batch = vec![batch[0], batch[1]];
        let new_batch = (expected_link, expected_range, expected_batch);
        expected_results.push(new_batch);

        let iter = Insert {
            node,
            batch,
            outdated_link_idx: vec![],
            split_second_half: None,
        };

        let results: Vec<_> = iter.collect();

        assert_eq!(results, expected_results);
        assert_eq!(node.links.len(), 1);
    }

    #[test]
    fn insert_iter_add_link_middle() {
        let keys = VecDeque::from(vec![50, 220, 280, 300, 400]);
        let indices = VecDeque::from(vec![0, 1, 3, 4, 5]);

        let batch = vec![
            (250, 0), // Add new link
            (251, 0), // Add new link
        ];

        let mut rng = Xoshiro256StarStar::from_entropy();

        let values: VecDeque<_> = (0..keys.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();
        let links: VecDeque<_> = (0..indices.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();

        let mut node = TreeNode::<u16, Cid> {
            layer: 1,
            keys: keys.clone(),
            values: values.clone(),
            indices: indices.clone(),
            links: links.clone(),
        };
        let node = &mut node;

        let batch: VecDeque<_> = batch
            .into_iter()
            .map(|(key, lvl)| (key, random_cid(&mut rng), lvl))
            .collect();

        let mut expected_results = Vec::with_capacity(1);

        //batch key 250, 251
        let expected_link = None;
        let expected_range = (Bound::Excluded(220), Bound::Excluded(280));
        let expected_batch = vec![batch[0], batch[1]];
        let new_batch = (expected_link, expected_range, expected_batch);
        expected_results.push(new_batch);

        let iter = Insert {
            node,
            batch,
            outdated_link_idx: vec![],
            split_second_half: None,
        };

        let results: Vec<_> = iter.collect();

        assert_eq!(results, expected_results);
        assert_eq!(node.links.len(), 5);
    }

    #[test]
    fn insert_iter_add_link_end() {
        let keys = VecDeque::from(vec![50, 220, 280, 300, 400]);
        let indices = VecDeque::from(vec![]);

        let batch = vec![
            (418, 0), // Add new link
            (419, 0), // Add new link
        ];

        let mut rng = Xoshiro256StarStar::from_entropy();

        let values: VecDeque<_> = (0..keys.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();
        let links: VecDeque<_> = (0..indices.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();

        let mut node = TreeNode::<u16, Cid> {
            layer: 1,
            keys: keys.clone(),
            values: values.clone(),
            indices: indices.clone(),
            links: links.clone(),
        };
        let node = &mut node;

        let batch: VecDeque<_> = batch
            .into_iter()
            .map(|(key, lvl)| (key, random_cid(&mut rng), lvl))
            .collect();

        let mut expected_results = Vec::with_capacity(1);

        //batch key 418, 419
        let expected_link = None;
        let expected_range = (Bound::Excluded(400), Bound::Unbounded);
        let expected_batch = vec![batch[0], batch[1]];
        let new_batch = (expected_link, expected_range, expected_batch);
        expected_results.push(new_batch);

        let iter = Insert {
            node,
            batch,
            outdated_link_idx: vec![],
            split_second_half: None,
        };

        let results: Vec<_> = iter.collect();

        assert_eq!(results, expected_results);
        assert!(node.links.is_empty());
    }

    #[test]
    fn insert_iter_add_kvs() {
        let keys = VecDeque::from(vec![50, 220, 280, 300, 400]);
        let indices = VecDeque::from(vec![0, 2, 5]);

        let batch = vec![
            (60, 1),  // Add KV to node
            (61, 1),  // Add KV to node
            (310, 1), // Add KV to node
            (390, 1), // Add KV to node
        ];

        let mut rng = Xoshiro256StarStar::from_entropy();

        let values: VecDeque<_> = (0..keys.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();
        let links: VecDeque<_> = (0..indices.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();

        let mut node = TreeNode::<u16, Cid> {
            layer: 1,
            keys: keys.clone(),
            values: values.clone(),
            indices: indices.clone(),
            links: links.clone(),
        };
        let node = &mut node;

        let batch: VecDeque<_> = batch
            .into_iter()
            .map(|(key, lvl)| (key, random_cid(&mut rng), lvl))
            .collect();

        let expected_results = Vec::with_capacity(0);

        let iter = Insert {
            node,
            batch,
            outdated_link_idx: vec![],
            split_second_half: None,
        };

        let results: Vec<_> = iter.collect();

        assert_eq!(results, expected_results);

        assert_eq!(node.keys[1], 60);
        assert_eq!(node.keys[2], 61);
        assert_eq!(node.keys[6], 310);
        assert_eq!(node.keys[7], 390);
    }

    #[test]
    fn insert_iter_medley() {
        let keys = VecDeque::from(vec![50, 220, 280, 300, 400]);
        let indices = VecDeque::from(vec![0, 2, 5]);

        let batch = vec![
            (20, 1),  // Split first link
            (60, 1),  // Add KV to node
            (250, 1), // Split second link
            (290, 0), // Add new link
            (310, 1), // Add KV to node
            (410, 0), // Add KV to link
        ];

        let mut rng = Xoshiro256StarStar::from_entropy();

        let values: VecDeque<_> = (0..keys.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();
        let links: VecDeque<_> = (0..indices.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();

        let mut node = TreeNode::<u16, Cid> {
            layer: 1,
            keys: keys.clone(),
            values: values.clone(),
            indices: indices.clone(),
            links: links.clone(),
        };
        let node = &mut node;

        let batch: VecDeque<_> = batch
            .into_iter()
            .map(|(key, lvl)| (key, random_cid(&mut rng), lvl))
            .collect();

        let mut expected_results = Vec::with_capacity(6);

        let expected_link = Some(links[0]);
        let expected_range = (Bound::Unbounded, Bound::Excluded(20));
        let expected_batch = vec![];
        let new_batch = (expected_link, expected_range, expected_batch);
        expected_results.push(new_batch);

        let expected_link = Some(links[0]);
        let expected_range = (Bound::Excluded(20), Bound::Excluded(50));
        let expected_batch = vec![];
        let new_batch = (expected_link, expected_range, expected_batch);
        expected_results.push(new_batch);

        let expected_link = Some(links[1]);
        let expected_range = (Bound::Excluded(220), Bound::Excluded(250));
        let expected_batch = vec![];
        let new_batch = (expected_link, expected_range, expected_batch);
        expected_results.push(new_batch);

        let expected_link = Some(links[1]);
        let expected_range = (Bound::Excluded(250), Bound::Excluded(280));
        let expected_batch = vec![];
        let new_batch = (expected_link, expected_range, expected_batch);
        expected_results.push(new_batch);

        //batch key 290
        let expected_link = None;
        let expected_range = (Bound::Excluded(280), Bound::Excluded(300));
        let expected_batch = vec![batch[3]];
        let new_batch = (expected_link, expected_range, expected_batch);
        expected_results.push(new_batch);

        //batch key 410
        let expected_link = Some(links[2]);
        let expected_range = (Bound::Excluded(400), Bound::Unbounded);
        let expected_batch = vec![batch[5]];
        let new_batch = (expected_link, expected_range, expected_batch);
        expected_results.push(new_batch);

        let iter = Insert {
            node,
            batch,
            outdated_link_idx: vec![],
            split_second_half: None,
        };

        let results: Vec<_> = iter.collect();

        assert_eq!(results, expected_results);
        assert!(node.links.is_empty());
    }

    #[test]
    fn remove_iter() {
        let keys = VecDeque::from(vec![
            /* link */ 50, 220, /* link */ 280, /* link */ 300, 400, /* link */
        ]);
        let indices = VecDeque::from(vec![0, 2, 3, 5]);

        let batch = vec![20, 50, 60, 230, 280, 290, 400, 410];
        let batch = VecDeque::from(batch);

        let mut rng = Xoshiro256StarStar::from_entropy();

        let values: VecDeque<_> = (0..keys.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();
        let links: VecDeque<_> = (0..indices.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();

        let mut node = TreeNode::<u16, Cid> {
            layer: 1,
            keys: keys.clone(),
            values: values.clone(),
            indices: indices.clone(),
            links: links.clone(),
        };
        let node = &mut node;

        let node_range = (Bound::Excluded(10), Bound::Excluded(450));

        let mut expected_results = Vec::with_capacity(3);

        //batch key 20
        let expected_links = vec![links[0]];
        let expected_range = (Bound::Excluded(10), Bound::Excluded(220));
        let expected_batch = vec![batch[0]];
        let new_batch = (expected_links, expected_range, expected_batch);
        expected_results.push(new_batch);

        //batch key 230, 290
        let expected_links = vec![links[1], links[2]];
        let expected_range = (Bound::Excluded(220), Bound::Excluded(300));
        let expected_batch = vec![batch[3], batch[5]];
        let new_batch = (expected_links, expected_range, expected_batch);
        expected_results.push(new_batch);

        //batch key 410
        let expected_links = vec![links[3]];
        let expected_range = (Bound::Excluded(300), Bound::Excluded(450));
        let expected_batch = vec![batch[7]];
        let new_batch = (expected_links, expected_range, expected_batch);
        expected_results.push(new_batch);

        let iter = Remove {
            node,
            node_range,
            batch,
        };

        let results: Vec<_> = iter.collect();

        assert_eq!(results, expected_results);
        assert!(node.links.is_empty());
    }

    fn random_cid(rng: &mut Xoshiro256StarStar) -> Cid {
        let mut input = [0u8; 64];
        rng.fill_bytes(&mut input);

        let hash = Sha512::new_with_prefix(input).finalize();

        let multihash = Multihash::wrap(0x13, &hash).unwrap();

        Cid::new_v1(/* DAG-CBOR */ 0x71, multihash)
    }
}

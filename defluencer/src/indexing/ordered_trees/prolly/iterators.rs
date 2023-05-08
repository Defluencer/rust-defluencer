use std::{
    collections::VecDeque,
    ops::{Bound, RangeBounds},
};

use super::node::{Branch, TreeNode, TreeNodeType};

use cid::Cid;

use crate::indexing::ordered_trees::traits::{Key, Value};

/// Split the batch into smaller batches with associated node links
pub struct Search<'a, K, T>
where
    T: TreeNodeType,
{
    pub node: &'a TreeNode<K, T>,
    pub batch: VecDeque<K>,
}

impl<'a, K: Key> Iterator for Search<'a, K, Branch> {
    type Item = (Cid, Vec<K>);

    fn next(&mut self) -> Option<Self::Item> {
        let mut keys = Vec::new();
        let mut link = None;
        while let Some(batch_key) = self.batch.front() {
            let idx = match self.node.keys.binary_search(&batch_key) {
                Ok(idx) => idx,
                Err(idx) => {
                    if idx == 0 {
                        self.batch.pop_front().unwrap();
                        continue;
                    }

                    // Since links are ordered, the previous one has the correct range.
                    idx - 1
                }
            };

            let Some(link) = link else {
                let batch_key = self.batch.pop_front().unwrap();
                keys.push(batch_key);
                link = Some(self.node.values.links[idx]);
                continue;
            };

            if link != self.node.values.links[idx] {
                return Some((link, keys));
            }

            let batch_key = self.batch.pop_front().unwrap();
            keys.push(batch_key);
        }

        if !keys.is_empty() && link.is_some() {
            return Some((link.unwrap(), keys));
        }

        None
    }
}

/// Split the batch into smaller batch with associated node links
pub struct Insert<'a, K, V, T>
where
    T: TreeNodeType,
{
    pub node: &'a TreeNode<K, T>,
    pub batch: VecDeque<(K, V)>,
}

impl<'a, K: Key, V: Value> Iterator for Insert<'a, K, V, Branch> {
    type Item = (Cid, Vec<(K, V)>);

    fn next(&mut self) -> Option<Self::Item> {
        let mut kvs = Vec::new();
        let mut link = None;
        while let Some((batch_key, _)) = self.batch.front() {
            let idx = match self.node.keys.binary_search(&batch_key) {
                Ok(idx) => idx,
                Err(idx) => {
                    if idx == 0 {
                        self.batch.pop_front().unwrap();
                        continue;
                    }

                    // Since links are ordered, the previous one has the correct range.
                    idx - 1
                }
            };

            let Some(link) = link else {
                let batch_item = self.batch.pop_front().unwrap();
                kvs.push(batch_item);
                link = Some(self.node.values.links[idx]);
                continue;
            };

            if link != self.node.values.links[idx] {
                return Some((link, kvs));
            }

            let batch_item = self.batch.pop_front().unwrap();
            kvs.push(batch_item);
        }

        if !kvs.is_empty() && link.is_some() {
            return Some((link.unwrap(), kvs));
        }

        None
    }
}

/// Split the batch into smaller batches with associated node links while removing batch keys
pub struct Remove<'a, K, T>
where
    T: TreeNodeType,
{
    pub node: &'a mut TreeNode<K, T>,
    pub batch: VecDeque<K>,
}

impl<'a, K: Key> Iterator for Remove<'a, K, Branch> {
    type Item = (Vec<Cid>, Vec<K>);

    fn next(&mut self) -> Option<Self::Item> {
        let mut range = None;
        let mut keys = Vec::new();
        let mut links = Vec::new();

        while let Some(batch_key) = self.batch.front() {
            let (idx, remove) = match self.node.keys.binary_search(batch_key) {
                Ok(idx) => (idx, true),
                Err(idx) => {
                    if idx == 0 {
                        let batch_key = self.batch.pop_front().unwrap();
                        if !keys.is_empty() {
                            keys.push(batch_key);
                        }
                        continue;
                    }

                    (idx - 1, false)
                }
            };

            if range.is_none() {
                let lb = if idx == 0 {
                    Bound::Unbounded
                } else {
                    let i = if remove { idx - 1 } else { idx };
                    Bound::Included(self.node.keys[i].clone())
                };

                let hb = if idx == self.node.keys.len() {
                    Bound::Unbounded
                } else {
                    match self.node.keys.get(idx + 1) {
                        Some(key) => Bound::Excluded(key.clone()),
                        None => Bound::Unbounded,
                    }
                };

                range = Some((lb, hb));
            }

            if !range.as_ref().unwrap().contains(&batch_key) {
                let (_, hb) = range.as_mut().unwrap();
                if remove && hb.as_ref() == Bound::Excluded(&self.node.keys[idx]) {
                    match self.node.keys.get(idx + 1) {
                        Some(key) => *hb = Bound::Excluded(key.clone()),
                        None => *hb = Bound::Unbounded,
                    }
                } else {
                    return Some((links, keys));
                }
            }

            let batch_key = self.batch.pop_front().unwrap();
            keys.push(batch_key);

            let link = if remove {
                //Include link to previous node for merging purpose
                if idx > 0 {
                    let link = self.node.values.links[idx - 1];
                    if !links.contains(&link) {
                        links.push(link);
                    }
                }

                self.node.keys.remove(idx).unwrap();
                self.node.values.links.remove(idx).unwrap()
            } else {
                self.node.values.links[idx]
            };

            if !links.contains(&link) {
                links.push(link);
            }
        }

        if !links.is_empty() && !keys.is_empty() {
            return Some((links, keys));
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
        let mut rng = Xoshiro256StarStar::from_entropy();

        let keys = VecDeque::from(vec![0, 3, 5, 7, 9, 10]);

        let links: VecDeque<_> = (0..keys.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();

        let mut node = TreeNode::<u8, Branch> {
            keys: keys.clone(),
            values: Branch {
                links: links.clone(),
            },
        };
        let node = &mut node;

        let batch = VecDeque::from(vec![0, 1, 2, 6, 7, 8, 9, 10]);

        let expected_link = links[0];
        let expected_kvs = vec![batch[0], batch[1], batch[2]];
        let batch_one = (expected_link, expected_kvs);

        let expected_link = links[2];
        let expected_kvs = vec![batch[3]];
        let batch_two = (expected_link, expected_kvs);

        let expected_link = links[3];
        let expected_kvs = vec![batch[4], batch[5]];
        let batch_three = (expected_link, expected_kvs);

        let expected_link = links[4];
        let expected_kvs = vec![batch[6]];
        let batch_four = (expected_link, expected_kvs);

        let expected_link = links[5];
        let expected_kvs = vec![batch[7]];
        let batch_five = (expected_link, expected_kvs);

        let expected_results = Vec::from(vec![
            batch_one,
            batch_two,
            batch_three,
            batch_four,
            batch_five,
        ]);

        let iter = Search {
            node,
            batch: batch.clone(),
        };

        let results: Vec<_> = iter.collect();

        assert_eq!(results, expected_results);
    }

    #[test]
    fn insert_iter() {
        let mut rng = Xoshiro256StarStar::from_entropy();

        let keys = VecDeque::from(vec![0, 3, 5, 7, 9, 10]);

        let links: VecDeque<_> = (0..keys.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();

        let mut node = TreeNode::<u8, Branch> {
            keys: keys.clone(),
            values: Branch {
                links: links.clone(),
            },
        };
        let node = &mut node;

        let batch = VecDeque::from(vec![0, 1, 2, 6, 7, 8, 9, 10]);
        let batch: VecDeque<_> = batch
            .into_iter()
            .map(|key| (key, random_cid(&mut rng)))
            .collect();

        let expected_link = links[0];
        let expected_kvs = vec![batch[0], batch[1], batch[2]];
        let batch_one = (expected_link, expected_kvs);

        let expected_link = links[2];
        let expected_kvs = vec![batch[3]];
        let batch_two = (expected_link, expected_kvs);

        let expected_link = links[3];
        let expected_kvs = vec![batch[4], batch[5]];
        let batch_three = (expected_link, expected_kvs);

        let expected_link = links[4];
        let expected_kvs = vec![batch[6]];
        let batch_four = (expected_link, expected_kvs);

        let expected_link = links[5];
        let expected_kvs = vec![batch[7]];
        let batch_five = (expected_link, expected_kvs);

        let expected_results = Vec::from(vec![
            batch_one,
            batch_two,
            batch_three,
            batch_four,
            batch_five,
        ]);

        let iter = Insert {
            node,
            batch: batch.clone(),
        };

        let results: Vec<_> = iter.collect();

        assert_eq!(results, expected_results);
    }

    #[test]
    fn remove_iter() {
        let mut rng = Xoshiro256StarStar::from_entropy();

        let keys = VecDeque::from(vec![0, 3, 5, 7, 9, 10]);

        let links: VecDeque<_> = (0..keys.len())
            .into_iter()
            .map(|_| random_cid(&mut rng))
            .collect();

        let mut node = TreeNode::<u8, Branch> {
            keys: keys.clone(),
            values: Branch {
                links: links.clone(),
            },
        };
        let node = &mut node;

        let batch = VecDeque::from(vec![0, 1, 2, 6, 7, 8, 9, 10]);

        let expected_keys = vec![0, 1, 2];
        let expected_links = vec![links[0]];
        let batch_one = (expected_links, expected_keys);

        let expected_keys = vec![6, 7, 8, 9, 10];
        let expected_links = vec![links[2], links[3], links[4], links[5]];
        let batch_two = (expected_links, expected_keys);

        let expected_results = Vec::from(vec![batch_one, batch_two]);

        let iter = Remove {
            node,
            batch: batch.clone(),
        };

        let results: Vec<_> = iter.collect();

        assert_eq!(results, expected_results);

        for (_, new_batch) in results {
            for batch_key in new_batch {
                assert!(!node.keys.contains(&batch_key));
            }
        }
    }

    fn random_cid(rng: &mut Xoshiro256StarStar) -> Cid {
        let mut input = [0u8; 64];
        rng.fill_bytes(&mut input);

        let hash = Sha512::new_with_prefix(input).finalize();

        let multihash = Multihash::wrap(0x13, &hash).unwrap();

        Cid::new_v1(/* DAG-CBOR */ 0x71, multihash)
    }

    /* fn unique_random_sorted_pairs(
        numb: usize,
        rng: &mut Xoshiro256StarStar,
    ) -> (VecDeque<u8>, VecDeque<Cid>) {
        let mut key_values = Vec::with_capacity(numb);

        for _ in 0..numb {
            let mut byte = [0];
            rng.fill_bytes(&mut byte);
            let key = byte[0];
            let link = random_cid(rng);

            key_values.push((key, link));
        }

        key_values.sort_unstable_by(|(a, _), (b, _)| a.cmp(&b));
        key_values.dedup_by(|(a, _), (b, _)| a == b);

        key_values.into_iter().unzip()
    } */

    /* fn unique_random_sorted_batch(numb: usize, rng: &mut Xoshiro256StarStar) -> VecDeque<u8> {
        let mut keys = Vec::with_capacity(numb);

        for _ in 0..numb {
            let mut byte = [0];
            rng.fill_bytes(&mut byte);
            let key = byte[0];

            keys.push(key);
        }

        keys.sort_unstable();
        keys.dedup();

        keys.into()
    } */
}

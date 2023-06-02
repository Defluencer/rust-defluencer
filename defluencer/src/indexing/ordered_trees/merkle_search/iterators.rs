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
    pub node_range: &'a (Bound<K>, Bound<K>),
    pub batch: VecDeque<(K, V, usize)>,
    pub link_ranges: VecDeque<(Cid, (Bound<K>, Bound<K>))>,
}

impl<'a, K: Key, V: Value> Iterator for Insert<'a, K, V> {
    type Item = Vec<(Option<Cid>, (Bound<K>, Bound<K>), Vec<(K, V, usize)>)>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut links: Vec<Option<Cid>> = vec![];
        let mut ranges: Vec<(Bound<K>, Bound<K>)> = vec![];
        let mut batches: Vec<Vec<(K, V, usize)>> = vec![];

        while let Some((batch_key, _, layer)) = self.batch.front() {
            // Drop out of range batch keys
            if !self.node_range.contains(batch_key) {
                self.batch.pop_front();
                continue;
            }

            // Check if this KVs go into this node
            if *layer == self.node.layer {
                //Case: batch has keys, range, link
                if let Some(range) = ranges.last_mut() {
                    //Case: batch key is IN the range => split lower node
                    if range.contains(batch_key) {
                        let (_, end) = range;
                        let new_end = Bound::Excluded(batch_key.clone());
                        let new_range = (new_end.clone(), end.clone());
                        *end = new_end;
                        ranges.push(new_range);

                        let link = links.last().unwrap();
                        links.push(*link);

                        batches.push(vec![]);

                        let (batch_key, value, _) = self.batch.pop_front().unwrap();

                        self.node.keys.push_back(batch_key);
                        self.node.values.push_back(value);

                        continue;
                    }

                    //Case: batch key is edge of range => update lower node
                    if range.end_bound() == Bound::Excluded(batch_key) {
                        let (batch_key, value, _) = self.batch.pop_front().unwrap();

                        self.node.keys.push_back(batch_key);
                        self.node.values.push_back(value);

                        let result: Vec<_> = links
                            .into_iter()
                            .zip(ranges.into_iter())
                            .zip(batches.into_iter())
                            .filter_map(|((link, range), batch)| {
                                if link.is_none() && batch.is_empty() {
                                    None
                                } else {
                                    Some((link, range, batch))
                                }
                            })
                            .collect();

                        return Some(result);
                    }

                    //Case: batch key is out of range
                    let result: Vec<_> = links
                        .into_iter()
                        .zip(ranges.into_iter())
                        .zip(batches.into_iter())
                        .filter_map(|((link, range), batch)| {
                            if link.is_none() && batch.is_empty() {
                                None
                            } else {
                                Some((link, range, batch))
                            }
                        })
                        .collect();

                    return Some(result);
                }

                //Case: batch has no range or link => create new lower node
                if !batches.is_empty() {
                    links = vec![None, None];

                    let range_1 = (
                        if self.node.keys.len() == 0 {
                            self.node_range.start_bound().cloned()
                        } else {
                            Bound::Excluded(self.node.keys[self.node.keys.len() - 1].clone())
                        },
                        Bound::Excluded(batch_key.clone()),
                    );

                    let range_2 = (
                        Bound::Excluded(batch_key.clone()),
                        self.node_range.end_bound().cloned(),
                    );

                    ranges = vec![range_1, range_2];

                    batches.push(vec![]);
                } else {
                    for i in 0..self.link_ranges.len() {
                        let (_, range) = &self.link_ranges[i];

                        if range.contains(batch_key) {
                            //Case: there's a link that need to be splitted

                            let (cid, range) = self.link_ranges.remove(i).unwrap();

                            links = vec![Some(cid), Some(cid)];

                            let (start, end) = range;
                            let range_1 = (start, Bound::Excluded(batch_key.clone()));
                            let range_2 = (Bound::Excluded(batch_key.clone()), end);

                            ranges = vec![range_1, range_2];

                            batches = vec![vec![], vec![]];

                            break;
                        }
                    }
                }

                let (batch_key, value, _) = self.batch.pop_front().unwrap();

                self.node.keys.push_back(batch_key);
                self.node.values.push_back(value);

                continue;
            }

            // Check if batch key update some links
            for i in 0..self.link_ranges.len() {
                let (_, range) = &self.link_ranges[i];

                if range.contains(batch_key) {
                    let (cid, range) = self.link_ranges.remove(i).unwrap();

                    links = vec![Some(cid)];
                    ranges = vec![range];

                    if batches.is_empty() {
                        batches = vec![vec![]];
                    } else {
                        batches.push(vec![]);
                    }

                    break;
                }
            }

            let batch_item = self.batch.pop_front().unwrap();

            match batches.last_mut() {
                Some(batch) => {
                    batch.push(batch_item);
                }
                None => batches = vec![vec![batch_item]],
            }
        }

        // Return the last batch
        if !batches.is_empty() {
            if links.is_empty() {
                links = vec![None];
            }

            if ranges.is_empty() {
                let range = (
                    Bound::Excluded(self.node.keys.back().unwrap().clone()),
                    self.node_range.end_bound().cloned(),
                );

                ranges = vec![range];
            }

            let result: Vec<_> = links
                .into_iter()
                .zip(ranges.into_iter())
                .zip(batches.into_iter())
                .filter_map(|((link, range), batch)| {
                    if link.is_none() && batch.is_empty() {
                        None
                    } else {
                        Some((link, range, batch))
                    }
                })
                .collect();

            return Some(result);
        }

        // insert remaining links
        for (link, range) in self.link_ranges.drain(..) {
            /* #[cfg(debug_assertions)]
            println!(
                "Reinsert Link\nIn Keys {:?}\nAt Range {:?}",
                self.node.keys, range
            ); */

            self.node
                .insert_link(link, (range.start_bound(), range.end_bound()));
        }

        None
    }
}

pub struct Remove<'a, K, V> {
    pub node: &'a mut TreeNode<K, V>,
    pub node_range: (Bound<K>, Bound<K>),
    pub batch: VecDeque<K>,
    pub key_rm_idx: Vec<usize>,
    pub indices_rm_idx: Vec<usize>,
}

impl<'a, K: Key, V: Value> Iterator for Remove<'a, K, V> {
    type Item = Vec<(Cid, (Bound<K>, Bound<K>), Vec<K>)>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut prev_link_idx = None;
        let mut prev_key_idx = None;
        let mut link: Option<Cid> = None;
        let mut range: Option<(Bound<K>, Bound<K>)> = None;
        let mut batch: Option<Vec<_>> = None;
        let mut batches = vec![];

        while let Some(batch_key) = self.batch.front() {
            /* #[cfg(debug_assertions)]
            println!("Key {:?}", batch_key); */

            let (key_idx, key_found) = match self.node.keys.binary_search(batch_key) {
                Ok(idx) => (idx, true),
                Err(idx) => (idx, false),
            };

            if key_found {
                // Check if there's already a batch
                let mut link_before = false;
                if link.is_some() && range.is_some() && batch.is_some() {
                    let link = link.take().unwrap();
                    let range = range.take().unwrap();
                    let batch = batch.take().unwrap();

                    /* #[cfg(debug_assertions)]
                    println!(
                        "Store\nBatch {:?}\nRange {:?}\nLink {:?}",
                        batch, range, link
                    ); */

                    batches.push((link, range, batch));

                    if prev_key_idx < Some(key_idx) {
                        /* #[cfg(debug_assertions)]
                        println!("Not same batch\nReturn"); */

                        return Some(batches);
                    }

                    link_before = true;
                }

                /* #[cfg(debug_assertions)]
                println!(
                    "Node contains key at index {}\nRemoving key from node",
                    key_idx
                ); */

                self.batch.pop_front().unwrap();

                self.key_rm_idx.push(key_idx);

                // Check link before this key
                if !link_before {
                    if let Ok(link_idx) = self.node.indices.binary_search(&key_idx) {
                        // Check for twin links
                        let mut twin_links = vec![link_idx];
                        if link_idx > 0
                            && self.node.indices[link_idx] == self.node.indices[link_idx - 1]
                            && !self.indices_rm_idx.contains(&(link_idx - 1))
                        {
                            twin_links.insert(0, link_idx - 1);
                        } else if link_idx < self.node.indices.len() - 1
                            && self.node.indices[link_idx] == self.node.indices[link_idx + 1]
                            && !self.indices_rm_idx.contains(&(link_idx + 1))
                        {
                            twin_links.push(link_idx + 1);
                        }

                        for link_idx in twin_links {
                            let link = self.node.links[link_idx];

                            /* #[cfg(debug_assertions)]
                            println!("New Link {}\nRemove Link Index {:?}", link, link_idx); */

                            self.indices_rm_idx.push(link_idx);

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

                            let range = (lb, hb);

                            /* #[cfg(debug_assertions)]
                            println!("New Range {:?}", range); */

                            let batch = vec![];

                            /* #[cfg(debug_assertions)]
                            println!(
                                "Store\nBatch {:?}\nRange {:?}\nLink {:?}",
                                batch, range, link
                            ); */

                            batches.push((link, range, batch));
                        }
                    }
                }

                // Check link after this key
                if let Ok(mut link_idx) = self.node.indices.binary_search(&(key_idx + 1)) {
                    // Check for twin links
                    let mut twin_link = None;
                    if link_idx > 0
                        && self.node.indices[link_idx] == self.node.indices[link_idx - 1]
                        && !self.indices_rm_idx.contains(&(link_idx - 1))
                    {
                        twin_link = Some(link_idx - 1);
                    } else if link_idx < self.node.indices.len() - 1
                        && self.node.indices[link_idx] == self.node.indices[link_idx + 1]
                        && !self.indices_rm_idx.contains(&(link_idx + 1))
                    {
                        twin_link = Some(link_idx);

                        // Always use the last index
                        link_idx += 1;
                    }

                    prev_link_idx = Some(link_idx);

                    // Add first twin link in batches
                    if let Some(first_twin_idx) = twin_link {
                        let link = self.node.links[first_twin_idx];

                        /* #[cfg(debug_assertions)]
                        println!(
                            "First twin Link {}\nRemove Link Index {:?}",
                            link, first_twin_idx
                        ); */

                        self.indices_rm_idx.push(first_twin_idx);

                        let lb = if (key_idx + 1) == 0 {
                            self.node_range.start_bound().cloned()
                        } else {
                            Bound::Excluded(self.node.keys[key_idx].clone())
                        };
                        let hb = if (key_idx + 1) >= self.node.keys.len() {
                            self.node_range.end_bound().cloned()
                        } else {
                            Bound::Excluded(self.node.keys[key_idx + 1].clone())
                        };

                        let range = (lb, hb);

                        /* #[cfg(debug_assertions)]
                        println!("New Range {:?}", range); */

                        let batch = vec![];

                        /* #[cfg(debug_assertions)]
                        println!(
                            "Store\nBatch {:?}\nRange {:?}\nLink {:?}",
                            batch, range, link
                        ); */

                        batches.push((link, range, batch));
                    }

                    let cid = self.node.links[link_idx];
                    link = Some(cid);

                    /* #[cfg(debug_assertions)]
                    println!("New Link {}", cid); */

                    if self.indices_rm_idx.binary_search(&link_idx).is_err() {
                        /* #[cfg(debug_assertions)]
                        println!("Remove Link Index {:?}", link_idx); */
                        self.indices_rm_idx.push(link_idx);
                    }

                    let lb = if (key_idx + 1) == 0 {
                        self.node_range.start_bound().cloned()
                    } else {
                        Bound::Excluded(self.node.keys[key_idx].clone())
                    };
                    let hb = if (key_idx + 1) >= self.node.keys.len() {
                        self.node_range.end_bound().cloned()
                    } else {
                        Bound::Excluded(self.node.keys[key_idx + 1].clone())
                    };
                    let r = (lb, hb);

                    /* #[cfg(debug_assertions)]
                    println!("New Range {:?}", r); */

                    range = Some(r);

                    batch = Some(vec![]);
                } else if !batches.is_empty() {
                    /* #[cfg(debug_assertions)]
                    println!("No link after key\nReturn"); */

                    return Some(batches);
                }

                prev_key_idx = Some(key_idx);

                continue;
            }

            let mut link_idx = match self.node.indices.binary_search(&key_idx) {
                Ok(idx) => idx,
                Err(_) => {
                    /* #[cfg(debug_assertions)]
                    println!("No link found"); */

                    self.batch.pop_front().unwrap();

                    continue;
                }
            };

            // Check for twin links
            let mut twin_link = None;
            if link_idx > 0 && self.node.indices[link_idx] == self.node.indices[link_idx - 1] {
                if !self.indices_rm_idx.contains(&(link_idx - 1)) {
                    twin_link = Some(link_idx - 1);
                }
            } else if link_idx < self.node.indices.len() - 1
                && self.node.indices[link_idx] == self.node.indices[link_idx + 1]
            {
                if !self.indices_rm_idx.contains(&link_idx) {
                    twin_link = Some(link_idx);
                }

                // Always use the last index
                link_idx += 1;
            }

            /* #[cfg(debug_assertions)]
            println!("Node contains link at index {}", link_idx); */

            if (prev_link_idx != Some(link_idx) || twin_link.is_some())
                && link.is_some()
                && range.is_some()
                && batch.is_some()
            {
                let link = link.take().unwrap();
                let range = range.take().unwrap();
                let batch = batch.take().unwrap();

                /* #[cfg(debug_assertions)]
                println!(
                    "Store\nBatch {:?}\nRange {:?}\nLink {:?}",
                    batch, range, link
                ); */

                let same_range = range.contains(&batch_key);

                batches.push((link, range, batch));

                if !same_range {
                    /* #[cfg(debug_assertions)]
                    println!("Not same range\nReturn"); */

                    return Some(batches);
                }
            }

            // Add other link in batches
            if let Some(first_twin_idx) = twin_link {
                let link = self.node.links[first_twin_idx];

                /* #[cfg(debug_assertions)]
                println!(
                    "First twin Link {}\nRemove Link Index {:?}",
                    link, first_twin_idx
                ); */

                self.indices_rm_idx.push(first_twin_idx);

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

                let range = (lb, hb);

                /* #[cfg(debug_assertions)]
                println!("New Range {:?}", range); */

                let batch = vec![];

                /* #[cfg(debug_assertions)]
                println!(
                    "Store\nBatch {:?}\nRange {:?}\nLink {:?}",
                    batch, range, link
                ); */

                batches.push((link, range, batch));
            }

            prev_link_idx = Some(link_idx);
            prev_key_idx = Some(key_idx);

            if link.is_none() {
                let cid = self.node.links[link_idx];

                link = Some(cid);

                /* #[cfg(debug_assertions)]
                println!("New Link {}", cid); */

                if self.indices_rm_idx.binary_search(&link_idx).is_err() {
                    /* #[cfg(debug_assertions)]
                    println!("Remove Link Index {:?}", link_idx); */
                    self.indices_rm_idx.push(link_idx);
                }
            }

            if range.is_none() {
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

                let r = (lb, hb);

                /* #[cfg(debug_assertions)]
                println!("New Range {:?}", r); */

                range = Some(r);
            }

            let batch_key = self.batch.pop_front().unwrap();

            match batch.as_mut() {
                Some(batch) => batch.push(batch_key),
                None => batch = Some(vec![batch_key]),
            }
        }

        if link.is_some() && range.is_some() && batch.is_some() {
            let link = link.take().unwrap();
            let range = range.take().unwrap();
            let batch = batch.take().unwrap();

            /* #[cfg(debug_assertions)]
            println!(
                "Store\nBatch {:?}\nRange {:?}\nLink {:?}\nReturn",
                batch, range, link
            ); */

            batches.push((link, range, batch));

            return Some(batches);
        }

        /* #[cfg(debug_assertions)]
        println!("Iter end"); */

        for &idx in self.key_rm_idx.iter().rev() {
            self.node.keys.remove(idx);
            self.node.values.remove(idx);
        }

        let mut count = self.key_rm_idx.len();
        if !self.indices_rm_idx.is_empty() {
            for i in (0..self.node.indices.len()).rev() {
                if self.indices_rm_idx.last().is_some() && i == *self.indices_rm_idx.last().unwrap()
                {
                    let idx = self.indices_rm_idx.pop().unwrap();

                    self.node.indices.remove(idx);
                    self.node.links.remove(idx);

                    if self.key_rm_idx.contains(&i) {
                        count = count.saturating_sub(1);
                    }
                } else {
                    self.node.indices[i] = self.node.indices[i].saturating_sub(count);
                }

                if count == 0 && self.indices_rm_idx.is_empty() {
                    break;
                }
            }
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

    /* #[test]
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
    fn insert_iter_empty_node() {
        let keys = VecDeque::from(vec![]);
        let indices = VecDeque::from(vec![]);

        let batch = vec![
            (290, 0), // Add new link
            (410, 0), // Add new link
            (420, 1), // Add new KV
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

        let expected_link = None;
        let expected_range = (Bound::Unbounded, Bound::Excluded(420));
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
    } */

    #[test]
    fn remove_iter_medley() {
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

        let node_range = (Bound::Excluded(10), Bound::Excluded(420));

        let mut expected_results = Vec::with_capacity(3);

        //batch key 20
        let expected_link = links[0];
        let expected_range = (node_range.start_bound().cloned(), Bound::Excluded(keys[0]));
        let expected_batch = vec![batch[0]];
        let new_batch = vec![(expected_link, expected_range, expected_batch)];
        expected_results.push(new_batch);

        //batch key 230
        let expected_link = links[1];
        let expected_range = (Bound::Excluded(keys[1]), Bound::Excluded(keys[2]));
        let expected_batch = vec![batch[3]];
        let mut new_batch = vec![(expected_link, expected_range, expected_batch)];
        //batch key 290
        let expected_link = links[2];
        let expected_range = (Bound::Excluded(keys[2]), Bound::Excluded(keys[3]));
        let expected_batch = vec![batch[5]];
        new_batch.push((expected_link, expected_range, expected_batch));
        expected_results.push(new_batch);

        //batch key 410
        let expected_link = links[3];
        let expected_range = (Bound::Excluded(keys[4]), node_range.end_bound().cloned());
        let expected_batch = vec![batch[7]];
        let new_batch = vec![(expected_link, expected_range, expected_batch)];
        expected_results.push(new_batch);

        let iter = Remove {
            node,
            node_range,
            batch,
            key_rm_idx: vec![],
            indices_rm_idx: vec![],
        };

        let results: Vec<_> = iter.collect();

        assert_eq!(results, expected_results);
        assert!(node.links.is_empty());
    }

    #[test]
    fn remove_iter_first_2_links() {
        let keys = VecDeque::from(vec![
            /* link */ 50, /* link */ 220, /* link */ 280, /* link */ 300,
            400, /* link */
        ]);
        let indices = VecDeque::from(vec![0, 1, 2, 3, 5]);

        let batch = vec![50];
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

        let node_range = (Bound::Unbounded, Bound::Unbounded);

        let mut expected_results = Vec::with_capacity(1);

        //batch key 50
        let expected_links = links[0];
        let expected_range = (Bound::Unbounded, Bound::Excluded(keys[0]));
        let expected_batch = vec![];
        let mut new_batch = vec![(expected_links, expected_range, expected_batch)];
        let expected_links = links[1];
        let expected_range = (Bound::Excluded(keys[0]), Bound::Excluded(keys[1]));
        let expected_batch = vec![];
        new_batch.push((expected_links, expected_range, expected_batch));
        expected_results.push(new_batch);

        let iter = Remove {
            node,
            node_range,
            batch,
            key_rm_idx: vec![],
            indices_rm_idx: vec![],
        };

        let results: Vec<_> = iter.collect();

        assert_eq!(results, expected_results);

        let indices = VecDeque::from(vec![1, 2, 4]);

        assert_eq!(node.indices, indices);
    }

    #[test]
    fn remove_iter_2_links() {
        let keys = VecDeque::from(vec![
            /* link */ 50, /* link */ 220, /* link */ 280, /* link */ 300,
            400, /* link */
        ]);
        let indices = VecDeque::from(vec![0, 1, 2, 3, 5]);

        let batch = vec![230, 240, 250, 280, 290, 291, 292];
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

        let node_range = (Bound::Unbounded, Bound::Unbounded);

        let mut expected_results = Vec::with_capacity(1);

        //batch key 280
        let expected_links = links[2];
        let expected_range = (Bound::Excluded(keys[1]), Bound::Excluded(keys[2]));
        let expected_batch = vec![batch[0], batch[1], batch[2]];
        let mut new_batch = vec![(expected_links, expected_range, expected_batch)];
        let expected_links = links[3];
        let expected_range = (Bound::Excluded(keys[2]), Bound::Excluded(keys[3]));
        let expected_batch = vec![batch[4], batch[5], batch[6]];
        new_batch.push((expected_links, expected_range, expected_batch));
        expected_results.push(new_batch);

        let iter = Remove {
            node,
            node_range,
            batch,
            key_rm_idx: vec![],
            indices_rm_idx: vec![],
        };

        let results: Vec<_> = iter.collect();

        assert_eq!(results, expected_results);

        let indices = VecDeque::from(vec![0, 1, 4]);

        assert_eq!(node.indices, indices);
    }

    #[test]
    fn remove_iter_last_2_links() {
        let keys = VecDeque::from(vec![
            /* link */ 50, /* link */ 220, /* link */ 280, /* link */ 300,
            /* link */ 400, /* link */
        ]);
        let mut indices = VecDeque::from(vec![0, 1, 2, 3, 4, 5]);

        let batch = vec![400];
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

        let node_range = (Bound::Excluded(10), Bound::Excluded(420));

        let mut expected_results = Vec::with_capacity(1);

        //batch key 400
        let expected_links = links[4];
        let expected_range = (Bound::Excluded(keys[3]), Bound::Excluded(keys[4]));
        let expected_batch = vec![];
        let mut new_batch = vec![(expected_links, expected_range, expected_batch)];
        let expected_links = links[5];
        let expected_range = (Bound::Excluded(keys[4]), node_range.end_bound().cloned());
        let expected_batch = vec![];
        new_batch.push((expected_links, expected_range, expected_batch));
        expected_results.push(new_batch);

        let iter = Remove {
            node,
            node_range,
            batch,
            key_rm_idx: vec![],
            indices_rm_idx: vec![],
        };

        let results: Vec<_> = iter.collect();

        assert_eq!(results, expected_results);

        indices.truncate(4);

        assert_eq!(node.indices, indices);
    }

    #[test]
    fn remove_iter_merged_node() {
        let keys = VecDeque::from(vec![
            /* link */ 3430, /* link */ 5676, /* double link */ 27716,
            /* link */ 33583, /* link */ 37957, /* link */
        ]);
        let indices = VecDeque::from(vec![0, 1, 2, 2, 3, 4, 5]);

        let batch = vec![10270, 23567, 35901, 44103, 46393];
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

        let node_range = (Bound::Unbounded, Bound::Excluded(46923));

        let mut expected_results = Vec::with_capacity(3);

        //batch key 10270, 23567
        let expected_links = links[2];
        let expected_range = (Bound::Excluded(keys[1]), Bound::Excluded(keys[2]));
        let expected_batch = vec![];
        let mut new_batch = vec![(expected_links, expected_range, expected_batch)];
        let expected_links = links[3];
        let expected_range = (Bound::Excluded(keys[1]), Bound::Excluded(keys[2]));
        let expected_batch = vec![batch[0], batch[1]];
        new_batch.push((expected_links, expected_range, expected_batch));
        expected_results.push(new_batch);

        //batch key 35901
        let expected_links = links[5];
        let expected_range = (Bound::Excluded(keys[3]), Bound::Excluded(keys[4]));
        let expected_batch = vec![batch[2]];
        let new_batch = vec![(expected_links, expected_range, expected_batch)];
        expected_results.push(new_batch);

        //batch key 44103, 46393
        let expected_links = links[6];
        let expected_range = (Bound::Excluded(keys[4]), node_range.end_bound().cloned());
        let expected_batch = vec![batch[3], batch[4]];
        let new_batch = vec![(expected_links, expected_range, expected_batch)];
        expected_results.push(new_batch);

        let iter = Remove {
            node,
            node_range,
            batch,
            key_rm_idx: vec![],
            indices_rm_idx: vec![],
        };

        let results: Vec<_> = iter.collect();

        assert_eq!(results, expected_results);

        let indices = vec![0, 1, 3];

        assert_eq!(node.indices, indices);
    }

    #[test]
    fn remove_iter_edge_case_2() {
        let keys = VecDeque::from(vec![
            /* link */ 27716, /* link */ 33583, /* link */ 37957, /* link */
        ]);
        let indices = VecDeque::from(vec![0, 1, 2, 3]);

        let batch = vec![11403, 16529, 20630, 23567, 33583, 40894];
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

        let node_range = (Bound::Excluded(10066), Bound::Excluded(46923));

        let mut expected_results = Vec::with_capacity(2);

        //batch key 11403..23567
        let expected_links = links[0];
        let expected_range = (node_range.start_bound().cloned(), Bound::Excluded(keys[0]));
        let expected_batch = vec![batch[0], batch[1], batch[2], batch[3]];
        let new_batch = vec![(expected_links, expected_range, expected_batch)];
        expected_results.push(new_batch);

        //batch key 33583
        let expected_links = links[1];
        let expected_range = (Bound::Excluded(keys[0]), Bound::Excluded(keys[1]));
        let expected_batch = vec![];
        let mut new_batch = vec![(expected_links, expected_range, expected_batch)];
        let expected_links = links[2];
        let expected_range = (Bound::Excluded(keys[1]), Bound::Excluded(keys[2]));
        let expected_batch = vec![];
        new_batch.push((expected_links, expected_range, expected_batch));
        expected_results.push(new_batch);

        //batch key 40894
        let expected_links = links[3];
        let expected_range = (Bound::Excluded(keys[2]), node_range.end_bound().cloned());
        let expected_batch = vec![batch[5]];
        let new_batch = vec![(expected_links, expected_range, expected_batch)];
        expected_results.push(new_batch);

        let iter = Remove {
            node,
            node_range,
            batch,
            key_rm_idx: vec![],
            indices_rm_idx: vec![],
        };

        let results: Vec<_> = iter.collect();

        assert_eq!(results, expected_results);
        assert!(node.indices.is_empty());
    }

    #[test]
    fn remove_iter_edge_case_3() {
        let keys = VecDeque::from(vec![
            /* link */ 3430, /* link */ 5676, /* link */
        ]);
        let indices = VecDeque::from(vec![0, 1, 2]);

        let batch = vec![4397, 5676];
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

        let node_range = (Bound::Unbounded, Bound::Excluded(10066));

        let mut expected_results = Vec::with_capacity(1);

        let expected_links = links[1];
        let expected_range = (Bound::Excluded(keys[0]), Bound::Excluded(keys[1]));
        let expected_batch = vec![batch[0]];
        let mut new_batch = vec![(expected_links, expected_range, expected_batch)];
        let expected_links = links[2];
        let expected_range = (Bound::Excluded(keys[1]), node_range.end_bound().cloned());
        let expected_batch = vec![];
        new_batch.push((expected_links, expected_range, expected_batch));
        expected_results.push(new_batch);

        let iter = Remove {
            node,
            node_range,
            batch,
            key_rm_idx: vec![],
            indices_rm_idx: vec![],
        };

        let results: Vec<_> = iter.collect();

        assert_eq!(results, expected_results);

        let indices = vec![0];

        assert_eq!(node.indices, indices);
    }

    #[test]
    fn remove_iter_edge_case_4() {
        let keys = VecDeque::from(vec![
            /* link */ 27716, /* link */ 33583, /* link */ 37957, /* link */
        ]);
        let indices = VecDeque::from(vec![0, 1, 2, 3]);

        let batch = vec![11095, 19836, 23920, 26017, 37957];
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

        let node_range = (Bound::Excluded(10066), Bound::Excluded(46923));

        let mut expected_results = Vec::with_capacity(2);

        //batch key 11095..26017
        let expected_links = links[0];
        let expected_range = (node_range.start_bound().cloned(), Bound::Excluded(keys[0]));
        let expected_batch = vec![batch[0], batch[1], batch[2], batch[3]];
        let new_batch = vec![(expected_links, expected_range, expected_batch)];
        expected_results.push(new_batch);

        //batch key 37957
        let expected_links = links[2];
        let expected_range = (Bound::Excluded(keys[1]), Bound::Excluded(keys[2]));
        let expected_batch = vec![];
        let mut new_batch = vec![(expected_links, expected_range, expected_batch)];
        let expected_links = links[3];
        let expected_range = (Bound::Excluded(keys[2]), node_range.end_bound().cloned());
        let expected_batch = vec![];
        new_batch.push((expected_links, expected_range, expected_batch));
        expected_results.push(new_batch);

        let iter = Remove {
            node,
            node_range,
            batch,
            key_rm_idx: vec![],
            indices_rm_idx: vec![],
        };

        let results: Vec<_> = iter.collect();

        assert_eq!(results, expected_results);

        let indices = vec![1];

        assert_eq!(node.indices, indices);
    }

    #[test]
    fn remove_iter_edge_case_5() {
        let keys = VecDeque::from(vec![
            /* link */ 27716, /* link */ 33583, /* link */ 37957, /* link */
        ]);
        let indices = VecDeque::from(vec![0, 1, 2, 3]);

        let batch = vec![
            11315, 19836, 23920, 25250, 27716, 31983, 40144, 42431, 44103,
        ];
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

        let node_range = (Bound::Excluded(10066), Bound::Excluded(46923));

        let mut expected_results = Vec::with_capacity(2);

        //batch key 11315..31983
        let expected_links = links[0];
        let expected_range = (node_range.start_bound().cloned(), Bound::Excluded(keys[0]));
        let expected_batch = vec![batch[0], batch[1], batch[2], batch[3]];
        let mut new_batch = vec![(expected_links, expected_range, expected_batch)];
        let expected_links = links[1];
        let expected_range = (Bound::Excluded(keys[0]), Bound::Excluded(keys[1]));
        let expected_batch = vec![batch[5]];
        new_batch.push((expected_links, expected_range, expected_batch));
        expected_results.push(new_batch);

        //batch key 40144..44103
        let expected_links = links[3];
        let expected_range = (Bound::Excluded(keys[2]), node_range.end_bound().cloned());
        let expected_batch = vec![batch[6], batch[7], batch[8]];
        let new_batch = vec![(expected_links, expected_range, expected_batch)];
        expected_results.push(new_batch);

        let iter = Remove {
            node,
            node_range,
            batch,
            key_rm_idx: vec![],
            indices_rm_idx: vec![],
        };

        let results: Vec<_> = iter.collect();

        assert_eq!(results, expected_results);

        let indices = vec![1];

        assert_eq!(node.indices, indices);
    }

    #[test]
    fn remove_iter_double_links() {
        let keys = VecDeque::from(vec![
            /* double link */ 27716, /* double link */ 33583,
            /* double link */ 37957, /* double link */
        ]);
        let indices = VecDeque::from(vec![0, 0, 1, 1, 2, 2, 3, 3]);

        let batch = vec![
            11095, 19836, 23920, 27717, 27718, 27719, 37954, 37955, 37956, 37958, 37959, 37960,
        ];
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

        let node_range = (Bound::Excluded(10066), Bound::Excluded(46923));

        let mut expected_results = Vec::with_capacity(2);

        //batch key 11095..23920
        let expected_links = links[0];
        let expected_range = (node_range.start_bound().cloned(), Bound::Excluded(keys[0]));
        let expected_batch = vec![];
        let mut new_batch = vec![(expected_links, expected_range, expected_batch)];
        let expected_links = links[1];
        let expected_range = (node_range.start_bound().cloned(), Bound::Excluded(keys[0]));
        let expected_batch = vec![batch[0], batch[1], batch[2]];
        new_batch.push((expected_links, expected_range, expected_batch));
        expected_results.push(new_batch);

        //batch key 27717..27719
        let expected_links = links[2];
        let expected_range = (Bound::Excluded(keys[0]), Bound::Excluded(keys[1]));
        let expected_batch = vec![];
        let mut new_batch = vec![(expected_links, expected_range, expected_batch)];
        let expected_links = links[3];
        let expected_range = (Bound::Excluded(keys[0]), Bound::Excluded(keys[1]));
        let expected_batch = vec![batch[3], batch[4], batch[5]];
        new_batch.push((expected_links, expected_range, expected_batch));
        expected_results.push(new_batch);

        //batch key 37954..37956
        let expected_links = links[4];
        let expected_range = (Bound::Excluded(keys[1]), Bound::Excluded(keys[2]));
        let expected_batch = vec![];
        let mut new_batch = vec![(expected_links, expected_range, expected_batch)];
        let expected_links = links[5];
        let expected_range = (Bound::Excluded(keys[1]), Bound::Excluded(keys[2]));
        let expected_batch = vec![batch[6], batch[7], batch[8]];
        new_batch.push((expected_links, expected_range, expected_batch));
        expected_results.push(new_batch);

        //batch key 37958..37960
        let expected_links = links[6];
        let expected_range = (Bound::Excluded(keys[2]), node_range.end_bound().cloned());
        let expected_batch = vec![];
        let mut new_batch = vec![(expected_links, expected_range, expected_batch)];
        let expected_links = links[7];
        let expected_range = (Bound::Excluded(keys[2]), node_range.end_bound().cloned());
        let expected_batch = vec![batch[9], batch[10], batch[11]];
        new_batch.push((expected_links, expected_range, expected_batch));
        expected_results.push(new_batch);

        let iter = Remove {
            node,
            node_range,
            batch,
            key_rm_idx: vec![],
            indices_rm_idx: vec![],
        };

        let results: Vec<_> = iter.collect();

        assert_eq!(results, expected_results);

        assert!(node.indices.is_empty());
    }

    fn random_cid(rng: &mut Xoshiro256StarStar) -> Cid {
        let mut input = [0u8; 64];
        rng.fill_bytes(&mut input);

        let hash = Sha512::new_with_prefix(input).finalize();

        let multihash = Multihash::wrap(0x13, &hash).unwrap();

        Cid::new_v1(/* DAG-CBOR */ 0x71, multihash)
    }
}

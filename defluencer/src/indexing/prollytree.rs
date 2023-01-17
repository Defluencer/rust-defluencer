use std::{
    collections::{hash_map::DefaultHasher, VecDeque},
    fmt::Debug,
    hash::{Hash, Hasher},
    ops::{Bound, RangeBounds},
    vec,
};

use async_recursion::async_recursion;

use futures::{
    channel::mpsc::{self, Sender},
    future::join_all,
    stream::{self, FuturesUnordered},
    Stream, StreamExt, TryStreamExt,
};

use either::Either;

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::types::IPLDLink;

use num::{BigUint, Integer, Zero};

use crate::errors::Error;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use sha2::{Digest, Sha512};

//TODO is it possible to use relative, instead of absolute, layer?

//TODO fugure out traits for keys. Other info go in config. This means can't use different hash algo?

//TODO Is async recursion inefficient? Could refactor but would be less readable.

//TODO Would it be better to have one batch operation that can insert AND remove? Too complex???

pub trait Key:
    Default
    + Debug
    + Clone
    + Copy
    + Eq
    + Ord
    + Hash
    + Serialize
    + DeserializeOwned
    + Send
    + Sync
    + Sized
{
}
impl<
        T: Default
            + Debug
            + Clone
            + Copy
            + Eq
            + Ord
            + Hash
            + Serialize
            + DeserializeOwned
            + Send
            + Sync
            + Sized,
    > Key for T
{
}

pub trait Value:
    Default + Debug + Clone + Copy + Eq + Serialize + DeserializeOwned + Send + Sync + Sized
{
}
impl<
        T: Default + Debug + Clone + Copy + Eq + Serialize + DeserializeOwned + Send + Sync + Sized,
    > Value for T
{
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct TreeNode<K, V> {
    layer: usize,

    base: usize, //TODO Move to config

    /// Keys and values sorted.
    key_values: VecDeque<(K, V)>, //TODO split into 2 vec

    /// Insert indexes into K.V. and links, sorted.
    index_links: VecDeque<(usize, IPLDLink)>, //TODO split into 2 vec
}

impl<K: Key, V: Value> TreeNode<K, V> {
    /// Insert sorted K-Vs into this node.
    ///
    /// Idempotent.
    fn batch_insert(
        &mut self,
        key_values: impl IntoIterator<Item = (K, V)> + Iterator<Item = (K, V)> + DoubleEndedIterator,
    ) {
        let mut stop = self.key_values.len();
        for (key, value) in key_values.rev() {
            for i in (0..stop).rev() {
                let node_key = self.key_values[i].0;

                if node_key < key {
                    self.key_values.insert(i + 1, (key, value));
                    stop = i + 1;
                    break;
                }

                if node_key == key {
                    self.key_values[i] = (key, value);
                    stop = i;
                    break;
                }
            }
        }
    }

    /// Remove from batch and node matching keys, merge batch ranges.
    fn batch_remove_match(&mut self, batch: &mut Batch<K, V>) {
        for j in (0..batch.elements.len()).rev() {
            let key = batch.elements[j].0;

            for i in (0..self.key_values.len()).rev() {
                let node_key = self.key_values[i].0;

                if node_key == key {
                    self.key_values.remove(i);
                    batch.elements.remove(j);

                    // Merge range before and after batch element
                    for k in 0..batch.ranges.len() - 1 {
                        let (l_low_b, l_up_b) = batch.ranges[k];

                        if j == 0 && l_low_b == Bound::Excluded(key) {
                            batch.ranges[k].0 = Bound::Unbounded;
                            break;
                        }

                        let (r_low_b, r_up_b) = batch.ranges[k + 1];

                        if l_up_b == Bound::Excluded(key) && r_low_b == Bound::Excluded(key) {
                            batch.ranges[k].1 = r_up_b;
                            batch.ranges.remove(k + 1);
                            break;
                        }

                        if j == (batch.elements.len() - 1) && r_up_b == Bound::Excluded(key) {
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
    fn insert_link(&mut self, link: IPLDLink, range: (Bound<K>, Bound<K>)) {
        for i in 0..self.key_values.len() {
            let mut low_b = Bound::Unbounded;

            if i != 0 {
                let key = self.key_values[i - 1].0;
                low_b = Bound::Excluded(key);
            }

            let mut up_b = Bound::Unbounded;

            if i != self.key_values.len() {
                let key = self.key_values[i].0;
                up_b = Bound::Excluded(key);
            }

            let inter_key_range = (low_b, up_b);

            if range_inclusion(inter_key_range, range) {
                match self.index_links.binary_search_by(|(idx, _)| idx.cmp(&i)) {
                    Ok(idx) => self.index_links[idx] = (i, link),
                    Err(idx) => self.index_links.insert(idx, (i, link)),
                }
            }
        }
    }

    /// Remove all K-Vs and links outside of range.
    ///
    /// Idempotent.
    fn rm_outrange(&mut self, range: (Bound<K>, Bound<K>)) {
        self.index_links.retain(|(idx, _)| {
            let low_b = {
                if *idx == 0 {
                    Bound::Unbounded
                } else {
                    match self.key_values.get(*idx - 1).map(|(key, _)| *key) {
                        Some(key) => Bound::Excluded(key),
                        None => Bound::Unbounded,
                    }
                }
            };

            let up_b = match self.key_values.get(*idx).map(|(key, _)| *key) {
                Some(key) => Bound::Excluded(key),
                None => Bound::Unbounded,
            };

            let link_range = (low_b, up_b);

            range_inclusion(range, link_range)
        });

        self.key_values.retain(|(key, _)| range.contains(&key));
    }

    /// Remove all elements. Returns keys, values and layers.
    fn rm_elements(&mut self) -> Vec<(K, V, usize)> {
        self.key_values
            .drain(..)
            .map(|(key, value)| (key, value, self.layer))
            .collect()
    }

    /// Remove all links and calculate each range bounds based on node keys.
    fn rm_link_ranges(&mut self) -> Vec<(IPLDLink, (Bound<K>, Bound<K>))> {
        self.index_links
            .drain(..)
            .map(|(idx, link)| {
                let low_b = {
                    if idx == 0 {
                        Bound::Unbounded
                    } else {
                        match self.key_values.get(idx - 1).map(|(key, _)| *key) {
                            Some(key) => Bound::Excluded(key),
                            None => Bound::Unbounded,
                        }
                    }
                };

                let up_b = match self.key_values.get(idx).map(|(key, _)| *key) {
                    Some(key) => Bound::Excluded(key),
                    None => Bound::Unbounded,
                };

                let range = (low_b, up_b);

                (link, range)
            })
            .collect()
    }

    /// Returns node elements and each link with range.
    fn into_inner(
        mut self,
    ) -> (
        Vec<(K, V, usize)>,
        Vec<(IPLDLink, (Bound<K>, Bound<K>))>,
        usize,
    ) {
        let link_ranges = self.rm_link_ranges();
        let elements = self.rm_elements();

        (elements, link_ranges, self.base)
    }

    /// Merge all elements and links of two nodes.
    fn merge(&mut self, other: Self) {
        let (elements, link_ranges, _) = other.into_inner();

        self.batch_insert(elements.into_iter().map(|(key, value, _)| (key, value)));

        for (link, range) in link_ranges {
            self.insert_link(link, range);
        }
    }
}

#[derive(Default, Debug, Clone)]
struct Batch<K, V> {
    pub elements: VecDeque<(K, V, usize)>,      // key, value, layer
    pub ranges: VecDeque<(Bound<K>, Bound<K>)>, // lower bound, upper bound
}

impl<K: Key, V: Value> Batch<K, V> {
    /// Insert sorted elements into this batch.
    ///
    /// Idempotent.
    fn batch_insert(
        &mut self,
        iter: impl IntoIterator<Item = (K, V, usize)>
            + Iterator<Item = (K, V, usize)>
            + DoubleEndedIterator,
    ) {
        let mut stop = self.elements.len();
        for (key, value, layer) in iter.rev() {
            for i in (0..stop).rev() {
                let batch_key = self.elements[i].0;

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
    fn rm_highest(&mut self) -> (VecDeque<(K, V)>, usize) {
        let highest_l = self
            .elements
            .iter()
            .fold(0, |state, (_, _, layer)| state.max(*layer));

        let mut rm_elements = VecDeque::with_capacity(self.elements.len());

        self.elements.retain(|(key, value, layer)| {
            let pred = *layer != highest_l;

            if !pred {
                for i in 0..self.ranges.len() {
                    let range = self.ranges[i];

                    if range.contains(key) {
                        let old_up_b = range.1;

                        self.ranges[i].1 = Bound::Excluded(*key);
                        let new_low_b = Bound::Excluded(*key);

                        let new_up_b = old_up_b;

                        // Empty range are not fine
                        if new_low_b != new_up_b {
                            let new_range = (new_low_b, new_up_b);

                            self.ranges.insert(i + 1, new_range);
                        }
                    }
                }

                rm_elements.push_back((*key, *value));
            }

            pred
        });

        (rm_elements, highest_l)
    }

    /// Split a multi-range batch into multiple single range batch.
    fn split_per_range(mut self) -> Vec<Self> {
        if self.ranges.len() < 2 {
            return Vec::default();
        }

        let mut batches = Vec::with_capacity(self.ranges.len());

        for range in self.ranges.into_iter() {
            let mut elements = VecDeque::with_capacity(self.elements.len());
            self.elements.retain(|(key, value, layer)| {
                let pred = !range.contains(key);

                if !pred {
                    elements.push_back((*key, *value, *layer));
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

pub(crate) async fn batch_get<K: Key, V: Value>(
    ipfs: &IpfsService,
    root: IPLDLink,
    mut keys: Vec<K>,
) -> impl Stream<Item = Result<(K, V), Error>> {
    keys.sort_unstable();

    let (tx, rx) = mpsc::channel(keys.len());

    execute_batch_get(ipfs.clone(), root, keys, tx).await;

    rx
}

#[async_recursion]
async fn execute_batch_get<K: Key, V: Value>(
    ipfs: IpfsService,
    link: IPLDLink,
    mut keys: Vec<K>,
    mut sender: Sender<Result<(K, V), Error>>,
) {
    let mut node = match ipfs.dag_get::<&str, TreeNode<K, V>>(link.link, None).await {
        Ok(n) => n,
        Err(e) => {
            let _ = sender.try_send(Err(e.into()));
            return;
        }
    };

    //Remove the keys present in this node and send the k-v pair.
    let mut stop = node.key_values.len();
    for j in (0..keys.len()).rev() {
        let key = keys[j];

        for i in (0..stop).rev() {
            let node_key = node.key_values[i].0;

            if node_key == key {
                let value = node.key_values[i].1;

                keys.remove(j);

                let _ = sender.try_send(Ok((key, value)));

                stop = i;
                break;
            }
        }
    }

    let link_ranges = node.rm_link_ranges();

    // Traverse to node that have keys in their range.
    let futures: Vec<_> = link_ranges
        .into_iter()
        .filter_map(|(link, range)| {
            for key in keys.iter() {
                if range.contains(key) {
                    let future =
                        execute_batch_get(ipfs.clone(), link, keys.clone(), sender.clone());

                    return Some(future);
                }
            }

            None
        })
        .collect();

    join_all(futures).await;
}

pub(crate) async fn batch_insert<K: Key, V: Value>(
    ipfs: &IpfsService,
    root: IPLDLink,
    key_values: Vec<(K, V)>,
) -> Result<IPLDLink, Error> {
    let root_node = ipfs
        .dag_get::<&str, TreeNode<K, V>>(root.link, None)
        .await?;

    let base = root_node.base;

    let mut elements: Vec<_> = key_values
        .into_iter()
        .map(|(key, value)| {
            let layer = calculate_layer(base, &key);
            (key, value, layer)
        })
        .collect();

    elements.sort_unstable_by(|(a, _, _), (b, _, _)| a.cmp(&b));

    let elements = VecDeque::from(elements);

    let range = (Bound::Unbounded, Bound::Unbounded);
    let ranges = VecDeque::from(vec![range]);

    let main_batch = Batch { elements, ranges };

    let (link, _) =
        execute_batch_insert::<K, V>(ipfs.clone(), Either::Left(root), main_batch).await?;

    return Ok(link);
}

#[async_recursion]
async fn execute_batch_insert<K: Key, V: Value>(
    ipfs: IpfsService,
    link: Either<IPLDLink, usize>, // Link OR base
    mut main_batch: Batch<K, V>,
) -> Result<(IPLDLink, (Bound<K>, Bound<K>)), Error> {
    let mut node = match link {
        Either::Left(ipld) => {
            ipfs.dag_get::<&str, TreeNode<K, V>>(ipld.link, None)
                .await?
        }
        Either::Right(base) => TreeNode {
            base,
            ..Default::default()
        },
    };

    // Get range for this node.
    let main_range = main_batch.ranges[0];

    // Remove node elements and links outside of batch range.
    node.rm_outrange(main_range);

    // Deconstruct node
    let (elements, link_ranges, base) = node.into_inner();

    // Insert node elements into batch,
    main_batch.batch_insert(elements.into_iter());

    // Split batch ranges around highest layer elements.
    let (key_values, layer) = main_batch.rm_highest();

    // Create node with highest layer elements only.
    let mut node = TreeNode {
        layer,
        base,
        key_values,
        index_links: VecDeque::default(),
    };

    // Split the batch into single range batches.
    let batches = main_batch.split_per_range();

    // Schedule each batch.
    let mut futures: FuturesUnordered<_> = FuturesUnordered::default();
    let mut modified_links = Vec::with_capacity(link_ranges.len());
    'batch: for batch in batches.into_iter() {
        let batch_range = batch.ranges[0];

        // If batch_range is included in link_range, attach link.
        for (i, (link, range)) in link_ranges.iter().enumerate() {
            if range_inclusion(*range, batch_range) {
                println!("{:?}", batch);

                let future = execute_batch_insert(ipfs.clone(), Either::Left(*link), batch);
                futures.push(future);

                modified_links.push(i);

                continue 'batch;
            }
        }

        // Drop empty batches.
        if !batch.elements.is_empty() {
            println!("{:?}", batch);

            let future = execute_batch_insert(ipfs.clone(), Either::Right(base), batch);
            futures.push(future);
            continue 'batch;
        }
    }

    // Execute batches.
    while let Some((link, range)) = futures.try_next().await? {
        // Insert links according to ranges from batch result.
        node.insert_link(link, range);
    }

    //Reinsert links that were not modified.
    for (i, (link, range)) in link_ranges.into_iter().enumerate() {
        if let Ok(idx) = modified_links.binary_search(&i) {
            if idx == i {
                continue;
            }
        }

        node.insert_link(link, range);
    }

    println!("Final {:?}", node);

    // Serialize node and add to ipfs.
    let cid = ipfs.dag_put(&node, Codec::DagCbor).await?;
    let link: IPLDLink = cid.into();

    // Return node link and range.
    return Ok((link, main_range));
}

pub(crate) async fn batch_remove<K: Key, V: Value>(
    ipfs: &IpfsService,
    root: IPLDLink,
    keys: Vec<K>,
) -> Result<Option<IPLDLink>, Error> {
    let root_node = ipfs
        .dag_get::<&str, TreeNode<K, V>>(root.link, None)
        .await?;

    let base = root_node.base;

    let mut elements: Vec<_> = keys
        .into_iter()
        .map(|key| {
            let layer = calculate_layer(base, &key);
            (key, V::default(), layer)
        })
        .collect();

    elements.sort_unstable_by(|(a, _, _), (b, _, _)| a.cmp(&b));

    let elements = VecDeque::from(elements);

    /* let mut ranges = VecDeque::with_capacity(elements.len() + 1);
    for i in 0..elements.len() {
        let range = if i == 0 {
            let key = elements[i].0;
            (Bound::Unbounded, Bound::Excluded(key))
        } else if i == elements.len() - 1 {
            let key = elements[i].0;
            (Bound::Excluded(key), Bound::Unbounded)
        } else {
            let low_b = {
                let key = elements[i - 1].0;
                Bound::Excluded(key)
            };

            let up_b = {
                let key = elements[i].0;
                Bound::Excluded(key)
            };

            (low_b, up_b)
        };

        ranges.push_back(range);
    } */

    let main_batch = Batch {
        elements,
        ranges: VecDeque::from(vec![(Bound::Unbounded, Bound::Unbounded)]),
    };

    let result = execute_batch_remove(ipfs.clone(), vec![root], main_batch).await?;

    let link = result.map(|(link, _)| link);

    return Ok(link);
}

#[async_recursion]
async fn execute_batch_remove<K: Key, V: Value>(
    ipfs: IpfsService,
    links: Vec<IPLDLink>,
    mut main_batch: Batch<K, V>,
) -> Result<Option<(IPLDLink, (Bound<K>, Bound<K>))>, Error> {
    let mut futures: FuturesUnordered<_> = links
        .into_iter()
        .map(|ipld| ipfs.dag_get::<&str, TreeNode<K, V>>(ipld.link, None))
        .collect();

    // Merge all the nodes
    let mut node = futures.try_next().await?.expect("Dag Get First Link");
    while let Some(new_node) = futures.try_next().await? {
        node.merge(new_node)
    }

    // Get range for this node.
    let main_range = main_batch.ranges[0];

    let link_ranges = node.rm_link_ranges();

    // Remove node and batch matching elements and merge batch ranges.
    node.batch_remove_match(&mut main_batch);

    // Split the batch into single range batches.
    let batches = main_batch.split_per_range();

    // Schedule each batch.
    let mut futures: FuturesUnordered<_> = FuturesUnordered::default();
    let mut modified_links = Vec::with_capacity(link_ranges.len());
    'batch: for batch in batches.into_iter() {
        let batch_range = batch.ranges[0];

        // If link_range is included in batch_range, attach link.
        let mut links = Vec::with_capacity(link_ranges.len());
        for (i, (link, range)) in link_ranges.iter().enumerate() {
            if range_inclusion(batch_range, *range) {
                links.push(*link);
                modified_links.push(i);
            }
        }

        if links.is_empty() {
            continue 'batch;
        }

        // One links can't be a merge and with no element can't change lower nodes.
        if links.len() == 1 && batch.elements.is_empty() {
            // Only multi-link or single link with element batch change the links.
            modified_links.pop();
            continue 'batch;
        }

        println!("{:?}", batch);

        let future = execute_batch_remove(ipfs.clone(), links, batch);
        futures.push(future);
    }

    if node.key_values.is_empty() {
        // This node is empty, it has max one link.
        if let Some(result) = futures.try_next().await? {
            if let Some((link, range)) = result {
                // Return the lower node since it's not empty.
                return Ok(Some((link, range)));
            }
        }

        return Ok(None);
    }

    // Execute batches.
    while let Some(result) = futures.try_next().await? {
        if let Some((link, range)) = result {
            // Insert links according to ranges from batch result.
            node.insert_link(link, range);
        }
    }

    // Reinsert links that were not modified.
    for (i, (link, range)) in link_ranges.into_iter().enumerate() {
        if let Ok(idx) = modified_links.binary_search(&i) {
            if idx == i {
                continue;
            }
        }

        node.insert_link(link, range);
    }

    println!("Final {:?}", node);

    // Serialize node and add to ipfs.
    let cid = ipfs.dag_put(&node, Codec::DagCbor).await?;
    let link: IPLDLink = cid.into();

    // Return node link and range.
    return Ok(Some((link, main_range)));
}

//TODO why static bounds ????
pub(crate) fn values<K: Key + 'static, V: Value + 'static>(
    ipfs: &IpfsService,
    root: IPLDLink,
) -> impl Stream<Item = Result<(K, V), Error>> + '_ {
    stream::try_unfold(Some(root), move |mut root| async move {
        let ipld = match root.take() {
            Some(ipld) => ipld,
            None => return Result::<_, Error>::Ok(None),
        };

        let root_node = ipfs
            .dag_get::<&str, TreeNode<K, V>>(ipld.link, None)
            .await?;

        let stream = stream_data(ipfs, root_node);

        Ok(Some((stream, root)))
    })
    .try_flatten()
}

//TODO why static bounds ????
fn stream_data<K: Key + 'static, V: Value + 'static>(
    ipfs: &IpfsService,
    node: TreeNode<K, V>,
) -> impl Stream<Item = Result<(K, V), Error>> + '_ {
    stream::try_unfold(
        (
            node.key_values.into_iter().enumerate().peekable(),
            node.index_links.into_iter().peekable(),
        ),
        move |(mut k_v_iter, mut links_iter)| async move {
            let (idx, _) = match links_iter.peek() {
                Some(p) => p,
                None => return Result::<_, Error>::Ok(None),
            };

            let (i, _) = match k_v_iter.peek() {
                Some(p) => p,
                None => return Result::<_, Error>::Ok(None),
            };

            // Stream the link if before the values.
            if idx == i {
                let (_, link) = links_iter.next().unwrap();

                let node = ipfs
                    .dag_get::<&str, TreeNode<K, V>>(link.link, None)
                    .await?;

                let stream = stream_data(ipfs, node).boxed_local();

                return Ok(Some((stream, (k_v_iter, links_iter))));
            } else {
                let (_, (key, value)) = k_v_iter.next().unwrap();

                let stream = stream::iter(vec![Ok((key, value))]).boxed_local();

                return Ok(Some((stream, (k_v_iter, links_iter))));
            }
        },
    )
    .try_flatten()
}

/// Using Horner's method but shortcircuit when first trailling non-zero is reached in the new base.
///
/// https://blogs.sas.com/content/iml/2022/09/12/convert-base-10.html
fn calculate_layer(base: usize, key: impl Hash) -> usize {
    let base = BigUint::from(base);

    //TODO Find good traits for hashing keys... instead of double hash
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    let hash = hasher.finish();

    let hash = Sha512::new_with_prefix(hash.to_be_bytes()).finalize();

    let hash_as_numb = BigUint::from_bytes_be(&hash); // Big endian because you treat the bits as a number reading it from left to right.

    let mut quotient = hash_as_numb;
    let mut remainder;

    let mut zero_count = 0;

    loop {
        (quotient, remainder) = quotient.div_rem(&base);

        if remainder != BigUint::zero() {
            break;
        }

        zero_count += 1;
    }

    zero_count
}

/// Is right range included in the left?
fn range_inclusion<T>(left_range: (Bound<T>, Bound<T>), right_range: (Bound<T>, Bound<T>)) -> bool
where
    T: PartialOrd,
{
    let (left_low_b, left_up_b) = left_range;
    let (right_low_b, right_up_b) = right_range;

    match (left_low_b, right_low_b, right_up_b, left_up_b) {
        (
            Bound::Excluded(left_low),
            Bound::Excluded(right_low),
            Bound::Excluded(right_up),
            Bound::Excluded(left_up),
        ) if left_low <= right_low && right_up <= left_up => true,

        (Bound::Unbounded, _, Bound::Excluded(right_up), Bound::Excluded(left_up))
            if right_up <= left_up =>
        {
            true
        }
        (Bound::Excluded(left_low), Bound::Excluded(right_low), _, Bound::Unbounded)
            if left_low <= right_low =>
        {
            true
        }
        (Bound::Unbounded, _, _, Bound::Unbounded) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    #![cfg(not(target_arch = "wasm32"))]

    use super::*;

    use rand_core::RngCore;

    use rand_xoshiro::{rand_core::SeedableRng, Xoshiro256StarStar};

    use sha2::{Digest, Sha512};

    use cid::Cid;

    use multihash::Multihash;

    fn random_cid(rng: &mut Xoshiro256StarStar) -> Cid {
        let mut input = [0u8; 64];
        rng.fill_bytes(&mut input);

        let hash = Sha512::new_with_prefix(input).finalize();

        let multihash = Multihash::wrap(0x13, &hash).unwrap();

        Cid::new_v1(/* DAG-CBOR */ 0x71, multihash)
    }

    #[test]
    fn test_zero_counting() {
        //let mut rng = Xoshiro256StarStar::seed_from_u64(2347867832489023);

        let base = 8;
        let zeros_to_find = 3;

        let mut key = 1usize;
        loop {
            //let key = random_cid(&mut rng);

            //let layer = calculate_layer(base, key.hash().digest());
            let layer = calculate_layer(base, &key.to_be_bytes());

            if layer == zeros_to_find {
                println!("{} Zero count in base {}", layer, base);

                //println!("Digest: {:?}", key.hash().digest());
                println!("Digest: {:?}", key.to_be_bytes());
                //let hash_as_numb = BigUint::from_bytes_be(key.hash().digest());
                let hash_as_numb = BigUint::from_bytes_be(&key.to_be_bytes());

                //let string = format!("{:#x}", hash_as_numb);
                let string = format!("{:#o}", hash_as_numb);

                //println!("Hex {}", string);
                println!("Octal {}", string);

                assert!(string.ends_with("000"));

                break;
            }

            key += 1;
        }
    }

    fn generate_simple_key_value_pairs<const LAYERS: usize>(
        base: usize,
        keys_per_layers: usize,
    ) -> Vec<(usize, usize)> {
        let mut key_values = Vec::with_capacity(keys_per_layers * LAYERS);
        let mut counters = [0; LAYERS];

        let mut key = 1usize;

        'outer: loop {
            let layer = calculate_layer(base, &key.to_be_bytes());

            'inner: for i in 0..LAYERS {
                if i == layer {
                    if counters[i] >= keys_per_layers {
                        break 'inner;
                    }

                    counters[i] += 1;

                    //println!("Layer {} Count {}", i, counters[i]);

                    key_values.push((key, 0));

                    if counters == [keys_per_layers; LAYERS] {
                        break 'outer;
                    }
                }
            }

            key += 1;
        }

        key_values
    }

    fn generate_cid_key_value_pairs<const LAYERS: usize>(
        base: usize,
        keys_per_layers: usize,
    ) -> Vec<(IPLDLink, IPLDLink)> {
        let mut key_values = Vec::with_capacity(keys_per_layers * LAYERS);
        let mut counters = [0; LAYERS];

        let mut rng = Xoshiro256StarStar::seed_from_u64(2347867832489023);

        'outer: loop {
            let random_key = random_cid(&mut rng);
            let layer = calculate_layer(base, &random_key.hash().digest());

            'inner: for i in 0..LAYERS {
                if i == layer {
                    if counters[i] >= keys_per_layers {
                        break 'inner;
                    }

                    counters[i] += 1;

                    println!("Layer {} Count {}", i, counters[i]);

                    let random_value = random_cid(&mut rng);
                    key_values.push((random_key.into(), random_value.into()));

                    if counters == [keys_per_layers; LAYERS] {
                        break 'outer;
                    }
                }
            }
        }

        key_values
    }

    async fn test_batch_insert(
        ipfs: &IpfsService,
        root: IPLDLink,
        key_values: Vec<(usize, usize)>,
    ) -> Result<IPLDLink, Error> {
        let root_node = ipfs
            .dag_get::<&str, TreeNode<usize, usize>>(root.link, None)
            .await?;

        let base = root_node.base;

        let mut elements: Vec<_> = key_values
            .into_iter()
            .map(|(key, value)| {
                let layer = calculate_layer(base, &key.to_be_bytes());
                (key, value, layer)
            })
            .collect();

        elements.sort_unstable_by(|(a, _, _), (b, _, _)| a.cmp(&b));

        let elements = VecDeque::from(elements);

        let range = (Bound::Unbounded, Bound::Unbounded);
        let ranges = VecDeque::from(vec![range]);

        let main_batch = Batch { elements, ranges };

        let (link, _) =
            execute_batch_insert::<usize, usize>(ipfs.clone(), Either::Left(root), main_batch)
                .await?;

        return Ok(link);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_batch_insert_idempotence() {
        const LAYERS: usize = 4;
        let keys_per_layers = 10;
        let base = 16;

        let key_values = generate_simple_key_value_pairs::<LAYERS>(base, keys_per_layers);

        println!("Keys & values generated!");

        let ipfs = IpfsService::default();

        let node = TreeNode::<[u8; 8], [u8; 1]> {
            base,
            ..Default::default()
        };

        let empty_root = ipfs
            .dag_put(&node, Codec::default())
            .await
            .expect("Empty Root Creation");
        let empty_root: IPLDLink = empty_root.into();

        let first_root = test_batch_insert(&ipfs, empty_root, key_values.clone())
            .await
            .expect("Tree Batch Write");

        println!("First tree root {}", first_root.link);

        let second_root = test_batch_insert(&ipfs, first_root, key_values.clone())
            .await
            .expect("Tree Batch Write");

        println!("Second tree root {}", second_root.link);

        assert_eq!(first_root, second_root);
    }

    async fn test_batch_remove(
        ipfs: &IpfsService,
        root: IPLDLink,
        keys: Vec<usize>,
    ) -> Result<Option<IPLDLink>, Error> {
        let root_node = ipfs
            .dag_get::<&str, TreeNode<usize, usize>>(root.link, None)
            .await?;

        let base = root_node.base;

        let mut elements: Vec<_> = keys
            .into_iter()
            .map(|key| {
                let layer = calculate_layer(base, &key.to_be_bytes());
                (key, 0, layer)
            })
            .collect();

        elements.sort_unstable_by(|(a, _, _), (b, _, _)| a.cmp(&b));

        let elements = VecDeque::from(elements);

        /* let mut ranges = VecDeque::with_capacity(elements.len() + 1);
        for i in 0..elements.len() {
            let range = if i == 0 {
                let key = elements[i].0;
                (Bound::Unbounded, Bound::Excluded(key))
            } else if i == elements.len() - 1 {
                let key = elements[i].0;
                (Bound::Excluded(key), Bound::Unbounded)
            } else {
                let low_b = {
                    let key = elements[i - 1].0;
                    Bound::Excluded(key)
                };

                let up_b = {
                    let key = elements[i].0;
                    Bound::Excluded(key)
                };

                (low_b, up_b)
            };

            ranges.push_back(range);
        } */

        let main_batch = Batch {
            elements,
            ranges: VecDeque::from(vec![(Bound::Unbounded, Bound::Unbounded)]),
        };

        //let main_batch = Batch { elements, ranges };

        println!("Start {:?}", main_batch);

        let result = execute_batch_remove(ipfs.clone(), vec![root], main_batch).await?;

        let link = result.map(|(link, _)| link);

        return Ok(link);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_batch_remove_all() {
        const LAYERS: usize = 4;
        let keys_per_layers = 10;
        let base = 16;

        let key_values = generate_simple_key_value_pairs::<LAYERS>(base, keys_per_layers);

        let ipfs = IpfsService::default();

        let link: IPLDLink =
            Cid::try_from("bafyreignwxdfgye6y56cufzybds4zjdx4hgxvipmxaq2ya7pmbylzsq44u")
                .unwrap()
                .into();
        let remove_batch: Vec<_> = key_values.into_iter().map(|(key, _)| key).collect();

        let second_root = test_batch_remove(&ipfs, link, remove_batch)
            .await
            .expect("Tree Batch Remove");

        assert_eq!(second_root, None);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_batch_remove_some() {
        let ipfs = IpfsService::default();

        /* const LAYERS: usize = 4;
        let keys_per_layers = 10;
        let base = 16;

        let key_values = generate_simple_key_value_pairs::<LAYERS>(base, keys_per_layers); */
        //Previously added
        let link: IPLDLink =
            Cid::try_from("bafyreignwxdfgye6y56cufzybds4zjdx4hgxvipmxaq2ya7pmbylzsq44u")
                .unwrap()
                .into();

        let remove_batch = vec![1, 2, 3];

        let second_root = test_batch_remove(&ipfs, link, remove_batch)
            .await
            .expect("Tree Batch Remove");

        println!("Second tree root {:?}", second_root);

        assert!(second_root.is_some());
    }
}

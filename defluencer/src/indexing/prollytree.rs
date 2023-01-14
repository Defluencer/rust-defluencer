use std::{
    collections::VecDeque,
    ops::{Bound, RangeBounds},
    vec,
};

use async_recursion::async_recursion;

use futures::{stream::FuturesUnordered, TryStreamExt};

use either::Either;

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::types::IPLDLink;

use multihash::MultihashGeneric;
type Multihash = MultihashGeneric<64>;

use cid::Cid;

use num::{BigUint, Integer, Zero};

use crate::errors::Error;

use serde::{Deserialize, Serialize};

//TODO is it possible to use relative layer instead of absolute, to save some bytes?

//TODO store only raw bytes of the hash as key. Other info go in config. This means can't use different hash algo?

//TODO Is async recursion inefficient? Could refactor but would be less readable.

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct TreeNode {
    layer: usize,

    base: usize, //TODO Move to config

    /// Keys and values sorted.
    key_values: VecDeque<(IPLDLink, IPLDLink)>,

    /// Insert indexes into K.V. and links, sorted.
    index_links: VecDeque<(usize, IPLDLink)>,
}

impl TreeNode {
    /// Insert sorted K-Vs into this node.
    ///
    /// Idempotent.
    fn batch_insert(
        &mut self,
        key_values: impl IntoIterator<Item = (IPLDLink, IPLDLink)>
            + Iterator<Item = (IPLDLink, IPLDLink)>
            + DoubleEndedIterator,
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

    /// Insert a link based on the range provided.
    ///
    /// Idempotent.
    fn insert_link(&mut self, link: IPLDLink, range: (Bound<IPLDLink>, Bound<IPLDLink>)) {
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

            if range_inclusion(range, inter_key_range) {
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
    fn rm_outrange(&mut self, range: (Bound<IPLDLink>, Bound<IPLDLink>)) {
        self.index_links.retain(|(idx, _)| {
            let low_b = match self.key_values.get(*idx - 1).map(|(key, _)| *key) {
                Some(key) => Bound::Excluded(key),
                None => Bound::Unbounded,
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
    fn rm_elements(&mut self) -> Vec<(IPLDLink, IPLDLink, usize)> {
        self.key_values
            .drain(..)
            .map(|(key, value)| (key, value, self.layer))
            .collect()
    }

    /// Remove all links and calculate each range bounds based on node keys.
    fn rm_link_ranges(&mut self) -> Vec<(IPLDLink, (Bound<IPLDLink>, Bound<IPLDLink>))> {
        self.index_links
            .drain(..)
            .map(|(idx, link)| {
                let low_b = match self.key_values.get(idx - 1).map(|(key, _)| *key) {
                    Some(key) => Bound::Excluded(key),
                    None => Bound::Unbounded,
                };

                let up_b = match self.key_values.get(idx).map(|(key, _)| *key) {
                    Some(key) => Bound::Excluded(key),
                    None => Bound::Unbounded,
                };

                let range = (low_b, up_b);

                (link, range)
            })
            .collect()

        //let mut ranges = Vec::with_capacity(self.index_links.len());

        /* for (idx, link) in self.index_links.into_iter() {
            let low_b = match self.key_values.get(idx - 1).map(|(key, _)| *key) {
                Some(key) => Bound::Excluded(key),
                None => Bound::Unbounded,
            };

            let up_b = match self.key_values.get(idx).map(|(key, _)| *key) {
                Some(key) => Bound::Excluded(key),
                None => Bound::Unbounded,
            };

            let range = (low_b, up_b);

            ranges.push((link, range));
        } */

        //ranges
    }

    /// Returns node elements and each link with range.
    fn into_inner(
        mut self,
    ) -> (
        Vec<(IPLDLink, IPLDLink, usize)>,
        Vec<(IPLDLink, (Bound<IPLDLink>, Bound<IPLDLink>))>,
        usize,
    ) {
        let link_ranges = self.rm_link_ranges();
        let elements = self.rm_elements();

        (elements, link_ranges, self.base)
    }

    /* /// Split off the node K-Vs into higher batch range
    fn split(&mut self, batch: &mut Batch) {
        let mut elements = Vec::with_capacity(self.key_values.len());
        self.key_values.retain(|(key, value)| {
            // The first range is the node range, others are splitting ranges.
            let pred = batch.ranges[0].contains(key);

            if !pred {
                elements.push((*key, *value, self.layer));
            }

            pred
        });

        batch.batch_insert(elements.into_iter());

        /* let iter = self
            .key_values
            .into_iter()
            .map(|(key, value)| (key, value, self.layer));

        batch.batch_insert(iter); */
    } */
}

/* pub(crate) async fn get(
    ipfs: &IpfsService,
    root: IPLDLink,
    key: Cid,
) -> Result<Option<Cid>, Error> {
    get_node(&ipfs, root, None, key).await
} */

/* #[async_recursion(?Send)]
async fn get_node(
    ipfs: &IpfsService,
    node: IPLDLink,
    layer: Option<usize>,
    key: Cid,
) -> Result<Option<Cid>, Error> {
    let node = ipfs.dag_get::<&str, TreeNode>(node.link, None).await?;

    let layer = match layer {
        Some(l) => l,
        None => calculate_layer(node.base, &key),
    };

    // Branch 0
    match node.keys.binary_search(&key.into()) {
        Ok(index) => {
            // Case A: Key is found at index.

            let value_idx = node.pointers[index];

            return Ok(Some(node.values[value_idx].link));
        }
        Err(index) => {
            // Case B: Key is not found, index is hypotetical ordering.

            // Branch 0.B.0
            if layer == node.layer {
                // Case A: Key should be on this layer.

                return Ok(None);
            }

            // Case B: Key should not be on this layer.

            if let Some(ipld) = node.link_at(index) {
                return get_node(&ipfs, ipld, Some(layer), key).await;
            }

            return Ok(None);
        }
    }
} */

/// Add multiple key value pair to the tree.
pub(crate) async fn batch_write<T>(
    ipfs: &IpfsService,
    root: IPLDLink,
    key_values: Vec<(IPLDLink, IPLDLink)>,
) -> Result<IPLDLink, Error> {
    let root_node = ipfs.dag_get::<&str, TreeNode>(root.link, None).await?;

    let base = root_node.base;

    let elements = key_values
        .into_iter()
        .map(|(key, value)| (key, value, calculate_layer(base, &(key.into()))))
        .collect();

    let range = (Bound::Unbounded, Bound::Unbounded);
    let ranges = VecDeque::from(vec![range]);

    let main_batch = Batch { elements, ranges };

    let (link, _) = execute_batch(ipfs, Either::Left(root), main_batch).await?;

    return Ok(link);
}

#[async_recursion(?Send)]
async fn execute_batch(
    ipfs: &IpfsService,
    link: Either<IPLDLink, usize>, // Link OR base
    mut main_batch: Batch,
) -> Result<(IPLDLink, (Bound<IPLDLink>, Bound<IPLDLink>)), Error> {
    let mut node = match link {
        Either::Left(ipld) => ipfs.dag_get::<&str, TreeNode>(ipld.link, None).await?,
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
    'batch: for batch in batches.into_iter() {
        let batch_range = batch.ranges[0];

        // If batch_range is included in link_range, attach link.
        for (link, range) in link_ranges.iter() {
            if range_inclusion(*range, batch_range) {
                let future = execute_batch(ipfs, Either::Left(*link), batch);
                futures.push(future);
                continue 'batch;
            }
        }

        // Drop empty batches.
        if !batch.elements.is_empty() {
            let future = execute_batch(ipfs, Either::Right(base), batch);
            futures.push(future);
            continue 'batch;
        }
    }

    // Execute batches.
    while let Some((link, range)) = futures.try_next().await? {
        // Insert links according to ranges from batch result.
        node.insert_link(link, range);
    }

    // Serialize node and add to ipfs.
    let cid = ipfs.dag_put(&node, Codec::DagCbor).await?;
    let link: IPLDLink = cid.into();

    // Return node link and range.
    return Ok((link, main_range));
}

#[derive(Debug, Clone)]
struct Batch {
    pub elements: VecDeque<(IPLDLink, IPLDLink, usize)>, // key, value, layer
    pub ranges: VecDeque<(Bound<IPLDLink>, Bound<IPLDLink>)>, // lower bound, upper bound
}

impl Batch {
    /// Insert sorted elements into this batch.
    ///
    /// Idempotent.
    fn batch_insert(
        &mut self,
        iter: impl IntoIterator<Item = (IPLDLink, IPLDLink, usize)>
            + Iterator<Item = (IPLDLink, IPLDLink, usize)>
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
    fn rm_highest(&mut self) -> (VecDeque<(IPLDLink, IPLDLink)>, usize) {
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

    /* /// Merge the other batch into self.
    fn merge(&mut self, other: Self) {
        let mut stop = other.elements.len();
        for (key, value, layer) in other.elements.into_iter().rev() {
            for i in (0..stop).rev() {
                let batch_key = self.elements[i].0;

                if batch_key < key {
                    self.elements.insert(i + 1, (key, value, layer));
                    stop = i + 1;
                    break;
                }
            }
        }
    } */

    /* /// Remove all element at layer and split the ranges.
    fn remove_layer(&mut self, rm_layer: usize) -> Vec<(IPLDLink, IPLDLink)> {
        let mut rm_elements = Vec::with_capacity(self.elements.len());

        self.elements.retain(|(key, value, layer)| {
            let pred = *layer != rm_layer;

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

                rm_elements.push((*key, *value));
            }

            pred
        });

        rm_elements
    } */

    /* /// Split into one batch per link.
    fn split_per_links(
        &mut self,
        link_ranges: Vec<(Bound<IPLDLink>, Bound<IPLDLink>)>,
    ) -> Vec<Self> {
        if link_ranges.is_empty() {
            return Vec::default();
        }

        let mut batches = Vec::with_capacity(self.ranges.len());

        for link_range in link_ranges.into_iter() {
            let mut elements = VecDeque::with_capacity(self.elements.len());
            self.elements.retain(|(key, value, layer)| {
                let pred = !link_range.contains(key);

                if !pred {
                    elements.push_back((*key, *value, *layer));
                }

                pred
            });

            let mut ranges = VecDeque::with_capacity(self.ranges.len());
            self.ranges.retain(|range| {
                let pred = !range_inclusion(link_range, *range);

                if !pred {
                    ranges.push_back(*range);
                }

                pred
            });

            let batch = Batch { elements, ranges };

            batches.push(batch);
        }

        batches
    } */
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
        (
            Bound::Included(left_low),
            Bound::Included(right_low),
            Bound::Included(right_up),
            Bound::Included(left_up),
        ) if left_low <= right_low && right_up <= left_up => true,
        (Bound::Unbounded, _, Bound::Excluded(right_up), Bound::Excluded(left_up))
            if right_up <= left_up =>
        {
            true
        }
        (Bound::Unbounded, _, Bound::Included(right_up), Bound::Included(left_up))
            if right_up <= left_up =>
        {
            true
        }
        (Bound::Unbounded, _, Bound::Included(right_up), Bound::Excluded(left_up))
            if right_up < left_up =>
        {
            true
        }
        (Bound::Unbounded, _, Bound::Excluded(right_up), Bound::Included(left_up))
            if right_up <= left_up =>
        {
            true
        }
        (Bound::Excluded(left_low), Bound::Excluded(right_low), _, Bound::Unbounded)
            if left_low <= right_low =>
        {
            true
        }
        (Bound::Included(left_low), Bound::Included(right_low), _, Bound::Unbounded)
            if left_low <= right_low =>
        {
            true
        }
        (Bound::Excluded(left_low), Bound::Included(right_low), _, Bound::Unbounded)
            if left_low < right_low =>
        {
            true
        }
        (Bound::Included(left_low), Bound::Excluded(right_low), _, Bound::Unbounded)
            if left_low <= right_low =>
        {
            true
        }
        (Bound::Unbounded, _, _, Bound::Unbounded) => true,
        _ => false,
    }
}

/* pub(crate) async fn insert(
    ipfs: &IpfsService,
    root: &mut IPLDLink,
    key: Cid,
    value: Cid,
) -> Result<Option<Cid>, Error> {
    put(&ipfs, root, key, value, None).await
} */

/* #[async_recursion(?Send)]
async fn put(
    ipfs: &IpfsService,
    node: &mut IPLDLink,
    key: Cid,
    value: Cid,
    layer: Option<usize>,
) -> Result<Option<Cid>, Error> {
    let mut tree_node = ipfs.dag_get::<&str, TreeNode>(node.link, None).await?;

    let layer = match layer {
        Some(l) => l,
        None => calculate_layer(tree_node.base, &key),
    };

    // Branch 0
    match tree_node.keys.binary_search(&key.into()) {
        Ok(index) => {
            // Case A: Key is found at index.

            let index = tree_node.pointers[index];

            let old_value = tree_node.values[index];
            tree_node.values[index] = value.into();

            // Serialize current node
            let new_cid = ipfs.dag_put(&tree_node, Codec::DagCbor).await?;
            let new_node: IPLDLink = new_cid.into();
            *node = new_node;

            return Ok(Some(old_value.link));
        }
        Err(index) => {
            // Case B: Key is not found, index is hypotetical ordering.

            // Branch 0.B.0
            if tree_node.layer == layer {
                // Case A: Key should be on this layer.

                tree_node.insert(index, true, key.into(), value.into());

                // Branch 0.B.0.A
                if let Some(ipld) = tree_node.link_at(index) {
                    // Case 0: Lower block present.

                    let mut lower_node = ipfs.dag_get::<&str, TreeNode>(ipld.link, None).await?;

                    // Index at which the key would be inserted.
                    let (split_index, exact) = match lower_node.keys.binary_search(&key.into()) {
                        Ok(index) => (index, true),
                        Err(index) => (index, false),
                    };

                    // Branch 0.B.0.A.0
                    if let Some(second_half_node) = lower_node.split_at(split_index, exact) {
                        // Case A: Key is in the middle of the lower block sequence.

                        let first_half_node = lower_node;

                        let (first_cid, second_cid) = futures_util::try_join!(
                            ipfs.dag_put(&first_half_node, Codec::default()),
                            ipfs.dag_put(&second_half_node, Codec::default())
                        )?;

                        let value_idx = tree_node.pointers[index];

                        // Update current node with rebalanced links
                        tree_node.values[value_idx - 1] = first_cid.into();
                        tree_node.values.insert(value_idx + 1, second_cid.into());

                        // Shift all pointers AFTER the insert point
                        for i in tree_node.pointers.iter_mut() {
                            if *i <= value_idx {
                                continue;
                            }

                            *i += 1;
                        }
                    }
                    // Case B: Key is before the lower block sequence.
                    // Case C: Key is after the lower block sequence.
                }

                // Case 1: No lower block.

                // Serialize current node
                let new_cid = ipfs.dag_put(&tree_node, Codec::DagCbor).await?;
                let new_node: IPLDLink = new_cid.into();
                *node = new_node;

                return Ok(None);
            }

            // Case B: Key should not be on this layer.

            // Branch 0.B.0.B
            if let Some(mut ipld) = tree_node.link_at(index) {
                // Case 0: Lower block present.

                let result = put(&ipfs, &mut ipld, key, value, Some(layer)).await?;

                //TODO check for split when recursing???

                if let Some(link) = tree_node.mut_link_at(index) {
                    *link = ipld;
                }

                // Serialize current node
                let new_cid = ipfs.dag_put(&tree_node, Codec::DagCbor).await?;
                let new_node: IPLDLink = new_cid.into();

                *node = new_node;

                return Ok(result);
            }

            // Case 1: No lower block.

            let new_node = TreeNode {
                layer,
                base: tree_node.base,
                keys: vec![key.into()],
                pointers: vec![0],
                values: vec![value.into()],
            };

            let new_cid = ipfs.dag_put(&new_node, Codec::DagCbor).await?;
            let new_node: IPLDLink = new_cid.into();

            tree_node.values.insert(index, new_node);

            // Shift all pointers AFTER the insert point
            for i in tree_node.pointers.iter_mut() {
                if *i < index {
                    continue;
                }

                *i += 1;
            }

            // Serialize current node
            let new_cid = ipfs.dag_put(&tree_node, Codec::DagCbor).await?;
            let new_node: IPLDLink = new_cid.into();

            *node = new_node;

            Ok(None)
        }
    }
} */

/* pub(crate) async fn remove(
    ipfs: &IpfsService,
    root: &mut IPLDLink,
    key: Cid,
) -> Result<Option<Cid>, Error> {
    todo!()
} */

/* pub(crate) fn iter(
    ipfs: &IpfsService,
    root: IPLDLink,
) -> impl Stream<Item = Result<(Cid, Cid), Error>> + '_ {
    todo!()
} */

/// Using Horner's method but shortcircuit when first trailling non-zero is reached in the new base.
///
/// https://blogs.sas.com/content/iml/2022/09/12/convert-base-10.html
fn calculate_layer(base: usize, key: &Cid) -> usize {
    let base = BigUint::from(base);

    let hash_as_numb = BigUint::from_bytes_be(key.hash().digest());
    // Big endian because you treat the bits as a number reading it from left to right.

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

#[cfg(test)]
mod tests {
    #![cfg(not(target_arch = "wasm32"))]

    use super::*;

    use rand_core::RngCore;

    use rand_xoshiro::{rand_core::SeedableRng, Xoshiro256StarStar};

    use sha2::{Digest, Sha512};

    fn random_cid(rng: &mut Xoshiro256StarStar) -> Cid {
        let mut input = [0u8; 64];
        rng.fill_bytes(&mut input);

        let hash = Sha512::new_with_prefix(input).finalize();

        let multihash = Multihash::wrap(0x13, &hash).unwrap();

        Cid::new_v1(/* DAG-CBOR */ 0x71, multihash)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_zero_counting() {
        let mut rng = Xoshiro256StarStar::seed_from_u64(2347867832489023);

        let base = 8;
        let zeros_to_find = 3;

        loop {
            let key = random_cid(&mut rng);

            let layer = calculate_layer(base, &key);

            if layer == zeros_to_find {
                println!("{} Zero count in base {}", layer, base);

                println!("Digest: {:?}", key.hash().digest());
                let hash_as_numb = BigUint::from_bytes_be(key.hash().digest());

                //let string = format!("{:#x}", hash_as_numb);
                let string = format!("{:#o}", hash_as_numb);

                //println!("Hex {}", string);
                println!("Octal {}", string);

                assert!(string.ends_with("000"));

                break;
            }
        }
    }
}

use async_recursion::async_recursion;

//use futures::{stream, Stream, StreamExt, TryStreamExt};

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::types::IPLDLink;

use multihash::MultihashGeneric;
type Multihash = MultihashGeneric<64>;

use cid::Cid;

use num::{BigUint, Integer, Zero};

use crate::errors::Error;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TreeNode {
    layer: usize, // TODO is it possible to use relative layer instead of absolute, to save some bytes?

    base: usize, // Move to config

    keys: Vec<IPLDLink>,   // Sorted keys
    pointers: Vec<usize>,  // Indexes for values associated with keys
    values: Vec<IPLDLink>, // Links to values AND nodes
}

impl TreeNode {
    /// Insert a new key when it comes after or in the middle of a lower block.
    pub fn insert_after(&mut self, index: usize, key: IPLDLink, value: IPLDLink) {
        self.keys.insert(index, key);

        let new_pointer = self.pointers[index];

        self.pointers.insert(index, new_pointer);

        // Shift all pointers AFTER the insert point
        for i in self.pointers.iter_mut() {
            if *i <= index {
                continue;
            }

            *i += 1;
        }

        self.values.insert(new_pointer, value);
    }

    /// Insert a new key when it comes before a lower block.
    pub fn insert_before(&mut self, index: usize, key: IPLDLink, value: IPLDLink) {
        self.keys.insert(index, key);

        let new_pointer = self.pointers[index];

        self.pointers.insert(index, new_pointer);

        // Shift all pointers AFTER the insert point
        for i in self.pointers.iter_mut() {
            if *i <= index {
                continue;
            }

            *i += 1;
        }

        self.values.insert(new_pointer - 1, value);
    }

    /// Return the link at the index if possible.
    pub fn link_at(&self, index: usize) -> Option<IPLDLink> {
        if let Some(idx) = self.pointers.get(index) {
            if index == 0 {
                if *idx == 0 {
                    return None;
                } else {
                    return Some(self.values[idx - 1]);
                }
            } else {
                let prev_idx = self.pointers[index - 1];
                let idx = self.pointers[index];

                if idx == prev_idx + 1 {
                    return None;
                } else {
                    return Some(self.values[idx - 1]);
                }
            }
        }

        self.values.get(index).map(|i| *i)
    }

    pub fn split_at(&mut self, index: usize) -> Option<Self> {
        if self.keys.is_empty() || index == 0 || index == self.keys.len() {
            return None;
        }

        // Second half, first value index.
        let idx = self.pointers[index];

        let keys = self.keys.drain(index..).collect();
        let pointers = self.pointers.drain(index..).collect();
        let values = self.values.drain(idx..).collect();

        Some(Self {
            layer: self.layer,
            base: self.base,
            keys,
            pointers,
            values,
        })
    }
}

pub(crate) async fn get(
    ipfs: &IpfsService,
    root: IPLDLink,
    key: Cid,
) -> Result<Option<Cid>, Error> {
    get_node(&ipfs, root, None, key).await
}

#[async_recursion(?Send)]
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
}

pub(crate) async fn insert(
    ipfs: &IpfsService,
    root: &mut IPLDLink,
    key: Cid,
    value: Cid,
) -> Result<Option<Cid>, Error> {
    put(&ipfs, root, key, value, None).await
}

#[async_recursion(?Send)]
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

                tree_node.insert_after(index, key.into(), value.into());

                // Branch 0.B.0.A
                if let Some(ipld) = tree_node.link_at(index) {
                    // Case 0: Lower block present.

                    let mut lower_node = ipfs.dag_get::<&str, TreeNode>(ipld.link, None).await?;

                    // Index at which the key would be inserted.
                    let split_index = lower_node
                        .keys
                        .binary_search(&key.into())
                        .unwrap_or_else(|x| x);

                    // Branch 0.B.0.A.0
                    if let Some(second_half_node) = lower_node.split_at(split_index) {
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
}

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

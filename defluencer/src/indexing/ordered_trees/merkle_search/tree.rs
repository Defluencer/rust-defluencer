use std::{
    collections::VecDeque,
    ops::{Bound, RangeBounds},
    vec,
};

use async_recursion::async_recursion;

use cid::Cid;

use futures::{
    stream::{self, FuturesUnordered},
    Stream, StreamExt, TryStreamExt,
};

use either::Either::{self, Right};

use ipfs_api::IpfsService;

use crate::indexing::ordered_trees::{
    errors::Error,
    traits::{Key, Value},
};

use super::{
    config::{calculate_layer, Config},
    node::{range_inclusion, Batch, TreeNode},
};

pub fn batch_get<K: Key, V: Value>(
    ipfs: IpfsService,
    root: Cid,
    keys: impl IntoIterator<Item = K>,
) -> impl Stream<Item = Result<(K, V), Error>> {
    let mut keys: Vec<_> = keys.into_iter().collect();
    keys.sort_unstable();

    search(ipfs, root, keys)
}

fn search<K: Key, V: Value>(
    ipfs: IpfsService,
    link: Cid,
    batch: Vec<K>,
) -> impl Stream<Item = Result<(K, V), Error>> {
    stream::once(async move {
        match ipfs.dag_get::<&str, TreeNode<K, V>>(link, None).await {
            Ok(node) => Ok((ipfs, node, batch)),
            Err(e) => Err(e),
        }
    })
    .map_ok(|(ipfs, node, batch)| {
        stream::iter(node.into_search_iter(batch))
            .map(move |either| match either {
                Either::Left((link, batch)) => search(ipfs.clone(), link, batch).boxed_local(),
                Right((key, value)) => stream::once(async move { Ok((key, value)) }).boxed_local(),
            })
            .flatten()
    })
    .try_flatten()
}

pub async fn batch_insert<K: Key, V: Value>(
    ipfs: IpfsService,
    root: Cid,
    config: Config,
    key_values: impl IntoIterator<Item = (K, V)>,
) -> Result<Cid, Error> {
    let elements: Result<Vec<_>, _> = key_values
        .into_iter()
        .map(|(key, value)| match calculate_layer(&config, key.clone()) {
            Ok(layer) => Ok((key, value, layer)),
            Err(e) => Err(e),
        })
        .collect();
    let mut elements = elements?;

    elements.sort_unstable_by(|(a, _, _), (b, _, _)| a.cmp(&b));

    let elements = VecDeque::from(elements);

    let range = (Bound::Unbounded, Bound::Unbounded);
    let ranges = VecDeque::from(vec![range]);

    let main_batch = Batch { elements, ranges };

    let (link, _) =
        execute_batch_insert::<K, V>(ipfs.clone(), Some(root), config, main_batch).await?;

    return Ok(link);
}

#[async_recursion]
async fn execute_batch_insert<K: Key, V: Value>(
    ipfs: IpfsService,
    link: Option<Cid>,
    config: Config,
    mut main_batch: Batch<K, V>,
) -> Result<(Cid, (Bound<K>, Bound<K>)), Error> {
    let mut node = match link {
        Some(cid) => ipfs.dag_get::<&str, TreeNode<K, V>>(cid, None).await?,
        None => TreeNode::default(),
    };

    // Get first range for this node.
    let main_range = main_batch.ranges[0].clone();

    // Remove node elements and links outside of batch range.
    node.rm_outrange((main_range.start_bound(), main_range.end_bound()));

    // Deconstruct node
    let (elements, link_ranges) = node.into_inner();

    // Insert node elements into batch,
    main_batch.batch_insert(elements.into_iter());

    // Split batch ranges around highest layer elements.
    let (keys, values, layer) = main_batch.rm_highest();

    // Create node with highest layer elements only.
    let mut node = TreeNode {
        layer,
        keys,
        values,
        indexes: VecDeque::default(),
        links: VecDeque::default(),
    };

    // Split the batch into single range batches.
    let batches = main_batch.split_per_range();

    // Schedule each batch.
    let mut futures: FuturesUnordered<_> = FuturesUnordered::default();
    let mut modified_links = Vec::with_capacity(link_ranges.len());
    'batch: for batch in batches.into_iter() {
        let batch_range = &batch.ranges[0];

        // If batch_range is included in link_range, attach link.
        for (i, (link, range)) in link_ranges.iter().enumerate() {
            if range_inclusion(
                (range.start_bound(), range.end_bound()),
                (batch_range.start_bound(), batch_range.end_bound()),
            ) {
                println!("{:?}", batch);

                let future = execute_batch_insert(ipfs.clone(), Some(*link), config.clone(), batch);
                futures.push(future);

                modified_links.push(i);

                continue 'batch;
            }
        }

        // Drop empty batches.
        if !batch.elements.is_empty() {
            println!("{:?}", batch);

            let future = execute_batch_insert(ipfs.clone(), None, config.clone(), batch);
            futures.push(future);
            continue 'batch;
        }
    }

    // Execute batches.
    while let Some((link, range)) = futures.try_next().await? {
        // Insert links according to ranges from batch result.
        node.insert_link(link, (range.start_bound(), range.end_bound()));
    }

    //Reinsert links that were not modified.
    for (i, (link, range)) in link_ranges.into_iter().enumerate() {
        if let Ok(idx) = modified_links.binary_search(&i) {
            if idx == i {
                continue;
            }
        }

        node.insert_link(link, (range.start_bound(), range.end_bound()));
    }

    println!("Final {:?}", node);

    // Serialize node and add to ipfs.
    let cid = ipfs.dag_put(&node, config.codec).await?;

    // Return node link and range.
    Ok((cid, main_range))
}

pub async fn batch_remove<K: Key, V: Value>(
    ipfs: IpfsService,
    root: Cid,
    config: Config,
    keys: impl IntoIterator<Item = K>,
) -> Result<Cid, Error> {
    let elements: Result<Vec<_>, _> = keys
        .into_iter()
        .map(|key| match calculate_layer(&config, key.clone()) {
            Ok(layer) => Ok((key, V::default(), layer)),
            Err(e) => Err(e),
        })
        .collect();
    let mut elements = elements?;

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

    let result = execute_batch_remove(ipfs.clone(), vec![root], config, main_batch).await?;
    let link = result.map(|(link, _)| link);

    Ok(link.unwrap())
}

#[async_recursion]
async fn execute_batch_remove<K: Key, V: Value>(
    ipfs: IpfsService,
    links: Vec<Cid>,
    config: Config,
    mut main_batch: Batch<K, V>,
) -> Result<Option<(Cid, (Bound<K>, Bound<K>))>, Error> {
    let mut futures: FuturesUnordered<_> = links
        .into_iter()
        .map(|cid| ipfs.dag_get::<&str, TreeNode<K, V>>(cid, None))
        .collect();

    // Merge all the nodes
    let mut node = futures.try_next().await?.expect("Dag Get First Link");
    while let Some(new_node) = futures.try_next().await? {
        node.merge(new_node)
    }

    // Get range for this node.
    let main_range = main_batch.ranges[0].clone();

    let link_ranges = node.rm_link_ranges();

    // Remove node and batch matching elements and merge batch ranges.
    node.batch_remove_match(&mut main_batch);

    // Split the batch into single range batches.
    let batches = main_batch.split_per_range();

    // Schedule each batch.
    let mut futures: FuturesUnordered<_> = FuturesUnordered::default();
    let mut modified_links = Vec::with_capacity(link_ranges.len());
    'batch: for batch in batches.into_iter() {
        let batch_range = &batch.ranges[0];

        // If link_range is included in batch_range, attach link.
        let mut links = Vec::with_capacity(link_ranges.len());
        for (i, (link, range)) in link_ranges.iter().enumerate() {
            if range_inclusion(
                (batch_range.start_bound(), batch_range.end_bound()),
                (range.start_bound(), range.end_bound()),
            ) {
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

        let future = execute_batch_remove(ipfs.clone(), links, config.clone(), batch);
        futures.push(future);
    }

    if node.keys.is_empty() {
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
            node.insert_link(link, (range.start_bound(), range.end_bound()));
        }
    }

    // Reinsert links that were not modified.
    for (i, (link, range)) in link_ranges.into_iter().enumerate() {
        if let Ok(idx) = modified_links.binary_search(&i) {
            if idx == i {
                continue;
            }
        }

        node.insert_link(link, (range.start_bound(), range.end_bound()));
    }

    println!("Final {:?}", node);

    // Serialize node and add to ipfs.
    let cid = ipfs.dag_put(&node, config.codec).await?;
    let link: Cid = cid.into();

    // Return node link and range.
    return Ok(Some((link, main_range)));
}

/// Stream all key value pairs in the tree in order.
pub fn stream_pairs<K: Key, V: Value>(
    ipfs: IpfsService,
    root: Cid,
) -> impl Stream<Item = Result<(K, V), Error>> {
    stream::once(async move {
        match ipfs.dag_get::<&str, TreeNode<K, V>>(root, None).await {
            Ok(node) => Ok((ipfs, node)),
            Err(e) => Err(e),
        }
    })
    .map_ok(move |(ipfs, node)| {
        stream::iter(node.into_iter())
            .map(move |either| match either {
                Either::Left((link, _)) => stream_pairs(ipfs.clone(), link).boxed_local(),
                Right((key, value)) => stream::once(async move { Ok((key, value)) }).boxed_local(),
            })
            .flatten()
    })
    .try_flatten()
}

/* #[cfg(test)]
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
    ) -> Vec<(Cid, Cid)> {
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
        root: Cid,
        key_values: Vec<(usize, usize)>,
    ) -> Result<Cid, Error> {
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
        let empty_root: Cid = empty_root.into();

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
        root: Cid,
        keys: Vec<usize>,
    ) -> Result<Option<Cid>, Error> {
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

        let link: Cid =
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
        let link: Cid =
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
} */

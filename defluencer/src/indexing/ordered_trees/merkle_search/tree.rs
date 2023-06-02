use crate::indexing::ordered_trees::{
    errors::Error,
    traits::{Key, Value},
};

use std::ops::{Bound, RangeBounds};

use super::{
    config::{calculate_layer, Config},
    node::TreeNode,
};

use async_recursion::async_recursion;

use cid::Cid;

use futures::{future::try_join_all, stream, Stream, StreamExt, TryStreamExt};

use either::Either::{self, Right};

use ipfs_api::{responses::Codec, IpfsService};

pub fn batch_get<K: Key, V: Value>(
    ipfs: IpfsService,
    root: Cid,
    codec: Codec,
    keys: impl IntoIterator<Item = K>,
) -> impl Stream<Item = Result<(K, V), Error>> {
    let mut keys: Vec<_> = keys.into_iter().collect();
    keys.sort_unstable();
    keys.dedup();

    search(ipfs, root, codec, keys)
}

fn search<K: Key, V: Value>(
    ipfs: IpfsService,
    link: Cid,
    codec: Codec,
    batch: Vec<K>,
) -> impl Stream<Item = Result<(K, V), Error>> {
    stream::once(async move {
        match ipfs
            .dag_get::<&str, TreeNode<K, V>>(link, None, codec)
            .await
        {
            Ok(node) => Ok((ipfs, node, batch)),
            Err(e) => Err(e),
        }
    })
    .map_ok(move |(ipfs, node, batch)| {
        stream::iter(node.into_search_iter(batch))
            .map(move |either| match either {
                Either::Left((link, batch)) => {
                    search(ipfs.clone(), link, codec, batch).boxed_local()
                }
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
    let mut batch = elements?;

    batch.sort_unstable_by(|(a, _, _), (b, _, _)| a.cmp(&b));
    batch.dedup_by(|(a, _, _), (b, _, _)| a == b);

    let range = (Bound::Unbounded, Bound::Unbounded);

    let option =
        execute_batch_insert::<K, V>(ipfs.clone(), config.clone(), Some(root), range, batch)
            .await?;

    let (link, _) = option.unwrap();

    Ok(link)
}

#[async_recursion]
async fn execute_batch_insert<K: Key, V: Value>(
    ipfs: IpfsService,
    config: Config,
    link: Option<Cid>,
    node_range: (Bound<K>, Bound<K>),
    mut batch: Vec<(K, V, usize)>,
) -> Result<Option<(Cid, (Bound<K>, Bound<K>))>, Error> {
    let futures: Vec<_> = link
        .into_iter()
        .map(|link| ipfs.dag_get::<&str, TreeNode<K, V>>(link, None, config.codec))
        .collect();

    let results: Result<Vec<_>, _> = try_join_all(futures).await;
    let results = results?;

    let mut link_ranges = vec![];
    for node in results {
        let (elements, l_r) = node.into_inner(&node_range);

        for (key, value, layer) in elements {
            if let Err(idx) = batch.binary_search_by(|(batch_key, _, _)| batch_key.cmp(&key)) {
                batch.insert(idx, (key, value, layer));
            }
        }

        link_ranges.extend(l_r);
    }

    let mut node = TreeNode::<K, V>::default();

    let futures: Vec<_> = node
        .insert_iter(&node_range, batch, link_ranges)
        .into_iter()
        .flatten()
        .map(|(link, range, batch)| {
            /* #[cfg(debug_assertions)]
            println!(
                "Link {:?}\nRange {:?}\nBatch {:?}",
                link,
                range,
                batch.iter().map(|(key, _, _)| key).collect::<Vec<_>>(),
            ); */

            execute_batch_insert(ipfs.clone(), config.clone(), link, range, batch)
        })
        .collect();

    let results = try_join_all(futures).await;
    let results = results?;

    for result in results {
        if let Some((link, range)) = result {
            /* #[cfg(debug_assertions)]
            println!(
                "Insert Link {}\nIn Keys {:?}\nAt Range {:?}",
                link, node.keys, range
            ); */

            node.insert_link(link, (range.start_bound(), range.end_bound()));
        }
    }

    if node.keys.is_empty() {
        // This node is empty, it has max one link.
        if let Some(link) = node.links.pop_back() {
            // Return the lower node since it's not empty.
            return Ok(Some((link, node_range)));
        }

        return Ok(None);
    }

    let cid = ipfs.dag_put(&node, config.codec, config.codec).await?;

    /* #[cfg(debug_assertions)]
    println!(
        "Final Node {}\nLayer {}\nRange {:?}\nKeys {:?}\nIndices {:?}",
        cid, node.layer, node_range, node.keys, node.indices
    ); */

    Ok(Some((cid, node_range)))
}

pub async fn batch_remove<K: Key, V: Value>(
    ipfs: IpfsService,
    root: Cid,
    config: Config,
    keys: impl IntoIterator<Item = K>,
) -> Result<Cid, Error> {
    let mut batch: Vec<_> = keys.into_iter().collect();

    if batch.is_empty() {
        return Ok(root);
    }

    batch.sort_unstable();

    let range = (Bound::Unbounded, Bound::Unbounded);

    let result = execute_batch_remove::<K, V>(
        ipfs.clone(),
        config.clone(),
        vec![root],
        vec![range],
        vec![batch],
    )
    .await?;

    let Some((link, _)) = result else {
        let empty_node = TreeNode::<K, V>::default();

        let root = ipfs
            .dag_put(&empty_node, config.codec, config.codec)
            .await?;

        return Ok(root);
    };

    Ok(link)
}

#[async_recursion]
async fn execute_batch_remove<K: Key, V: Value>(
    ipfs: IpfsService,
    config: Config,
    links: Vec<Cid>,
    ranges: Vec<(Bound<K>, Bound<K>)>,
    batches: Vec<Vec<K>>,
) -> Result<Option<(Cid, (Bound<K>, Bound<K>))>, Error> {
    let futures: Vec<_> = links
        .iter()
        .map(|cid| ipfs.dag_get::<&str, TreeNode<K, V>>(*cid, None, config.codec))
        .collect();

    let results = try_join_all(futures).await;
    let mut results = results?;

    let h_lvl = results.iter().fold(0, |acc, node| acc.max(node.layer));

    for i in 0..results.len() {
        if results[i].layer < h_lvl {
            let link = links[i];

            let mut empty_node = TreeNode::<K, V>::default();
            empty_node.layer = h_lvl;
            empty_node.indices.push_back(0);
            empty_node.links.push_back(link);

            results[i] = empty_node;
        }
    }

    let mut node = TreeNode::<K, V>::default();
    let mut range = (Bound::Unbounded, Bound::Unbounded);

    for (new_node, new_range) in results.into_iter().zip(ranges.into_iter()) {
        node.merge(&range, new_node, &new_range);

        range = match (range, new_range) {
            ((Bound::Unbounded, Bound::Excluded(prev_end)), (_, Bound::Excluded(next_end))) => {
                (Bound::Unbounded, Bound::Excluded(next_end.max(prev_end)))
            }
            (
                (Bound::Excluded(prev_start), Bound::Excluded(prev_end)),
                (Bound::Excluded(next_start), Bound::Excluded(next_end)),
            ) => (
                Bound::Excluded(next_start.min(prev_start)),
                Bound::Excluded(next_end.max(prev_end)),
            ),
            ((Bound::Excluded(prev_start), Bound::Unbounded), (Bound::Excluded(next_start), _)) => {
                (
                    Bound::Excluded(next_start.min(prev_start)),
                    Bound::Unbounded,
                )
            }
            _ => (Bound::Unbounded, Bound::Unbounded),
        };
    }

    let batch: Vec<_> = batches.into_iter().flatten().collect();

    let mut futures = Vec::new();
    for batches in node.remove_iter(range.clone(), batch) {
        let mut links = Vec::with_capacity(batches.len());
        let mut ranges = Vec::with_capacity(batches.len());
        let mut keys = Vec::with_capacity(batches.len());

        for (link, range, batch) in batches {
            /* #[cfg(debug_assertions)]
            println!("Batch {:?}\nRange {:?}\nLink {:?}", batch, range, link); */

            links.push(link);
            ranges.push(range);
            keys.push(batch);
        }

        let fut = execute_batch_remove::<K, V>(ipfs.clone(), config.clone(), links, ranges, keys);

        futures.push(fut);
    }

    let results = try_join_all(futures).await;
    let mut results = results?;

    if node.keys.is_empty() {
        // This node is empty, it has max one link.
        if let Some(result) = results.pop() {
            if let Some((link, range)) = result {
                // Return the lower node since it's not empty.
                return Ok(Some((link, range)));
            }
        }

        return Ok(None);
    }

    for result in results {
        if let Some((link, range)) = result {
            node.insert_link(link, (range.start_bound(), range.end_bound()));
        }
    }

    /* #[cfg(debug_assertions)]
    println!(
        "Final Node\nLayer {}\nRange {:?}\nKeys {:?}\nIndices {:?}",
        node.layer, range, node.keys, node.indices
    ); */

    let cid = ipfs.dag_put(&node, config.codec, config.codec).await?;

    Ok(Some((cid, range)))
}

/// Stream all key value pairs in the tree in order.
pub fn stream_pairs<K: Key, V: Value>(
    ipfs: IpfsService,
    root: Cid,
    codec: Codec,
) -> impl Stream<Item = Result<(K, V), Error>> {
    stream::once(async move {
        match ipfs
            .dag_get::<&str, TreeNode<K, V>>(root, None, codec)
            .await
        {
            Ok(node) => Ok((ipfs, node)),
            Err(e) => Err(e),
        }
    })
    .map_ok(move |(ipfs, node)| {
        stream::iter(node.into_iter())
            .map(move |either| match either {
                Either::Left((link, _)) => stream_pairs(ipfs.clone(), link, codec).boxed_local(),
                Right((key, value)) => stream::once(async move { Ok((key, value)) }).boxed_local(),
            })
            .flatten()
    })
    .try_flatten()
}

#[cfg(test)]
mod tests {
    #![cfg(not(target_arch = "wasm32"))]

    use super::*;

    use rand::Rng;
    use rand_core::RngCore;

    use rand_xoshiro::{rand_core::SeedableRng, Xoshiro256StarStar};

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn tree_stream_all() {
        let mut rng = Xoshiro256StarStar::seed_from_u64(6784236783546783546u64);
        let ipfs = IpfsService::default();

        let mut config = Config::default();
        config.base = 2;

        let empty_node = TreeNode::<u16, DataBlob>::default();

        let root = ipfs
            .dag_put(&empty_node, config.codec, config.codec)
            .await
            .expect("Empty Node");

        println!("Empty Root {}", root);

        let batch = unique_random_sorted_pairs::<32>(100, &mut rng);

        let tree_cid =
            batch_insert::<u16, DataBlob>(ipfs.clone(), root, config.clone(), batch.clone())
                .await
                .expect("Batch Insert");

        println!("New Root {}", tree_cid);

        let result: Vec<_> = stream_pairs::<u16, DataBlob>(ipfs, tree_cid, config.codec)
            .collect()
            .await;
        let results: Result<Vec<_>, Error> = result.into_iter().collect();
        let result = results.expect("Tree Streaming");

        assert_eq!(result, batch);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn tree_batch_get() {
        let mut rng = Xoshiro256StarStar::seed_from_u64(6784236783546783546u64);
        let ipfs = IpfsService::default();

        let mut config = Config::default();
        config.base = 2;

        //Run first test to generate the mst
        let tree_cid =
            Cid::try_from("bafyreiejcj45rskegzvo6fn6a6q6nrwcwilxbqnh44p2325huwhyrrdl2i").unwrap();

        let batch = unique_random_sorted_pairs::<32>(100, &mut rng);

        let mut rng = Xoshiro256StarStar::from_entropy();

        // 10 random KVs
        let mut batch: Vec<_> = (0..10)
            .map(|_| batch[rng.gen_range(0..batch.len())].clone())
            .collect();

        batch.sort_unstable_by(|(key, _), (other, _)| key.cmp(other));
        batch.dedup_by(|(key, _), (other, _)| key == other);

        let keys: Vec<_> = batch.clone().into_iter().map(|(key, _)| key).collect();

        println!("Get keys {:?}", keys);

        let result: Vec<_> = batch_get::<u16, DataBlob>(ipfs, tree_cid, config.codec, keys)
            .collect()
            .await;
        let results: Result<Vec<_>, Error> = result.into_iter().collect();
        let result = results.expect("Tree Batch Get");

        assert_eq!(result, batch);
    }

    #[tokio::test]
    #[ignore]
    async fn tree_batch_insert() {
        let mut rng = Xoshiro256StarStar::seed_from_u64(6784236783546783546u64);
        let ipfs = IpfsService::default();

        let mut config = Config::default();
        config.base = 2;

        let original_batch = unique_random_sorted_pairs::<32>(100, &mut rng);

        //Run first test to generate the prolly tree
        let tree_cid =
            Cid::try_from("bafyreiejcj45rskegzvo6fn6a6q6nrwcwilxbqnh44p2325huwhyrrdl2i").unwrap();

        let mut rng = Xoshiro256StarStar::from_entropy();

        let batch = unique_random_sorted_pairs::<32>(10, &mut rng);

        println!(
            "Test Insert Keys {:?}",
            batch.iter().map(|(key, _)| *key).collect::<Vec<_>>()
        );

        let tree_cid =
            batch_insert::<u16, DataBlob>(ipfs.clone(), tree_cid, config.clone(), batch.clone())
                .await
                .expect("Empty tree");

        println!("Result {}", tree_cid);

        let keys: Vec<_> = batch.clone().into_iter().map(|(key, _)| key).collect();

        let result: Vec<_> =
            batch_get::<u16, DataBlob>(ipfs.clone(), tree_cid, config.codec, keys.clone())
                .collect()
                .await;
        let results: Result<Vec<_>, Error> = result.into_iter().collect();
        let result = results.expect("Tree Batch Get");

        assert_eq!(result, batch);

        let result: Vec<_> = stream_pairs::<u16, DataBlob>(ipfs, tree_cid, config.codec)
            .collect()
            .await;
        let results: Result<Vec<_>, Error> = result.into_iter().collect();
        let result = results.expect("Tree Streaming");

        let result_keys: Vec<_> = result.into_iter().map(|(key, _)| key).collect();

        let mut batch_keys: Vec<_> = original_batch.into_iter().map(|(key, _)| key).collect();
        batch_keys.extend(keys.into_iter());
        batch_keys.sort_unstable();
        batch_keys.dedup();

        assert_eq!(result_keys, batch_keys);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn tree_remove_all() {
        let mut rng = Xoshiro256StarStar::seed_from_u64(6784236783546783546u64);
        let ipfs = IpfsService::default();

        let mut config = Config::default();
        config.base = 2;

        let batch = unique_random_sorted_pairs::<32>(100, &mut rng);
        let (keys, _): (Vec<_>, Vec<_>) = batch.into_iter().unzip();

        //Run first test to generate the prolly tree
        let empty_tree_cid =
            Cid::try_from("bafyreibhg6tzrxlknugy5zkqs6da3cftxt7mi7rpvfb7lkbubfqcc63fmm").unwrap();
        let tree_cid =
            Cid::try_from("bafyreiejcj45rskegzvo6fn6a6q6nrwcwilxbqnh44p2325huwhyrrdl2i").unwrap();

        let result = batch_remove::<u16, DataBlob>(ipfs, tree_cid, config, keys)
            .await
            .expect("Empty tree");

        assert_eq!(result, empty_tree_cid);
    }

    #[tokio::test]
    #[ignore]
    async fn tree_remove_some() {
        let mut rng = Xoshiro256StarStar::seed_from_u64(6784236783546783546u64);
        let ipfs = IpfsService::default();

        let mut config = Config::default();
        config.base = 2;

        let mut batch = unique_random_sorted_pairs::<32>(100, &mut rng);

        //Run first test to generate the prolly tree
        let tree_cid =
            Cid::try_from("bafyreiejcj45rskegzvo6fn6a6q6nrwcwilxbqnh44p2325huwhyrrdl2i").unwrap();

        let mut rng = Xoshiro256StarStar::from_entropy();

        // 10 random KVs
        let mut keys = Vec::with_capacity(10);

        for _ in 0..10 {
            let (key, _) = batch.remove(rng.gen_range(0..batch.len()));
            keys.push(key);
        }

        keys.sort_unstable();
        keys.dedup();

        /* let keys = vec![
            11315, 19836, 23920, 25250, 27716, 31983, 40144, 42431, 44103, 57053,
        ];

        for other in keys.iter() {
            let idx = batch.binary_search_by(|(k, _)| k.cmp(other)).unwrap();
            batch.remove(idx);
        } */

        println!("Test Remove Keys {:?}", keys);

        let tree_cid =
            batch_remove::<u16, DataBlob>(ipfs.clone(), tree_cid, config.clone(), keys.clone())
                .await
                .expect("Empty tree");

        println!("Result {}", tree_cid);

        let result: Vec<_> = batch_get::<u16, DataBlob>(ipfs.clone(), tree_cid, config.codec, keys)
            .collect()
            .await;
        let results: Result<Vec<_>, Error> = result.into_iter().collect();
        let result = results.expect("Tree Batch Get");

        assert!(result.is_empty(), "Result {:?}", result);

        let result: Vec<_> = stream_pairs::<u16, DataBlob>(ipfs, tree_cid, config.codec)
            .collect()
            .await;
        let results: Result<Vec<_>, Error> = result.into_iter().collect();
        let result = results.expect("Tree Streaming");

        let (result_keys, _): (Vec<_>, Vec<_>) = result.into_iter().unzip();
        let (batch_keys, _): (Vec<_>, Vec<_>) = batch.into_iter().unzip();

        assert_eq!(result_keys, batch_keys);
    }

    type DataBlob = Vec<u8>;

    fn unique_random_sorted_pairs<const T: usize>(
        numb: usize,
        rng: &mut Xoshiro256StarStar,
    ) -> Vec<(u16, DataBlob)> {
        let mut key_values = Vec::with_capacity(numb);

        for _ in 0..numb {
            let key = rng.next_u32() as u16;
            let mut value = [0u8; T];
            rng.fill_bytes(&mut value);

            key_values.push((key, value.to_vec()));
        }

        key_values.sort_unstable_by(|(a, _), (b, _)| a.cmp(&b));
        key_values.dedup_by(|(a, _), (b, _)| a == b);

        key_values
    }
}

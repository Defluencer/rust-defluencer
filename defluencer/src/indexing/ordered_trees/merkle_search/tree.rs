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

    let range = (Bound::Unbounded, Bound::Unbounded);

    let (link, _) =
        execute_batch_insert::<K, V>(ipfs.clone(), Some(root), config.clone(), range, batch)
            .await?;

    Ok(link)
}

#[async_recursion]
async fn execute_batch_insert<K: Key, V: Value>(
    ipfs: IpfsService,
    mut node_link: Option<Cid>,
    config: Config,
    range: (Bound<K>, Bound<K>),
    batch: Vec<(K, V, usize)>,
) -> Result<(Cid, (Bound<K>, Bound<K>)), Error> {
    let layer = batch
        .iter()
        .fold(0, |state, (_, _, layer)| state.max(*layer));

    let mut node = TreeNode {
        layer,
        ..Default::default()
    };

    if let Some(link) = node_link {
        let temp = ipfs
            .dag_get::<&str, TreeNode<K, V>>(link, None, config.codec)
            .await?;
        if temp.layer == layer {
            node = temp;
            node_link = None;
        }
    };

    // Splitting a node is trimming copies with different ranges.
    node.crop((range.start_bound(), range.end_bound()));

    let futures: Vec<_> = node
        .insert_iter(batch.into_iter())
        .map(|(batch_link, range, batch)| {
            let link = match (batch_link, node_link) {
                (None, None) => None,
                (None, Some(i)) => Some(i),
                (Some(i), None) => Some(i),
                (Some(_), Some(_)) => panic!(
                    "Nodes are either new (no batch links) or the node link was consumed to get the node"
                ),
            };
            execute_batch_insert(ipfs.clone(), link, config.clone(), range, batch)
        })
        .collect();

    let results: Result<Vec<_>, _> = try_join_all(futures).await;
    let results = results?;

    for (link, range) in results {
        node.insert_link(link, (range.start_bound(), range.end_bound()));
    }

    let cid = ipfs.dag_put(&node, config.codec, config.codec).await?;

    Ok((cid, range))
}

pub async fn batch_remove<K: Key, V: Value>(
    ipfs: IpfsService,
    root: Cid,
    config: Config,
    keys: impl IntoIterator<Item = K>,
) -> Result<Cid, Error> {
    let mut batch: Vec<_> = keys.into_iter().collect();
    batch.sort_unstable();

    let range = (Bound::Unbounded, Bound::Unbounded);

    let result =
        execute_batch_remove::<K, V>(ipfs.clone(), vec![root], config, range, batch).await?;
    let link = result.map(|(link, _)| link);

    Ok(link.unwrap())
}

#[async_recursion]
async fn execute_batch_remove<K: Key, V: Value>(
    ipfs: IpfsService,
    links: Vec<Cid>,
    config: Config,
    range: (Bound<K>, Bound<K>),
    batch: Vec<K>,
) -> Result<Option<(Cid, (Bound<K>, Bound<K>))>, Error> {
    let futures: Vec<_> = links
        .into_iter()
        .map(|cid| ipfs.dag_get::<&str, TreeNode<K, V>>(cid, None, config.codec))
        .collect();

    let results = try_join_all(futures).await;
    let results = results?;

    let mut node = results
        .into_iter()
        .reduce(|mut node, new| {
            node.merge(new);
            node
        })
        .unwrap();

    let futures: Vec<_> = node
        .remove_iter(range.clone(), batch)
        .map(|(links, range, batch)| {
            execute_batch_remove::<K, V>(ipfs.clone(), links, config.clone(), range, batch)
        })
        .collect();

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

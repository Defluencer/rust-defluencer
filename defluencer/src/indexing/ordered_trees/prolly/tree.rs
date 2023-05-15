use async_recursion::async_recursion;

use futures::{future::try_join_all, stream, Stream, StreamExt, TryStreamExt};

use ipfs_api::{responses::Codec, IpfsService};

use super::{
    deserialization::TreeNodes,
    node::{Branch, Leaf, TreeNode},
    Config,
};

use cid::Cid;

use crate::indexing::ordered_trees::{
    errors::Error,
    traits::{Key, Value},
};

/// Stream all the KVs that correspond with the keys in batch.
pub fn batch_get<K: Key, V: Value>(
    ipfs: IpfsService,
    root: Cid,
    codec: Codec,
    keys: impl IntoIterator<Item = K>,
) -> impl Stream<Item = Result<(K, V), Error>> {
    let mut batch = keys.into_iter().collect::<Vec<_>>();

    batch.sort_unstable();
    batch.dedup();

    stream::once(async move {
        match ipfs
            .dag_get::<&str, TreeNodes<K, V>>(root, None, codec)
            .await
        {
            Ok(node) => Ok((ipfs, node, batch)),
            Err(e) => Err(e),
        }
    })
    .map_ok(move |(ipfs, node, batch)| match node {
        TreeNodes::Branch(branch) => search_branch(ipfs, branch, codec, batch).boxed_local(),
        TreeNodes::Leaf(leaf) => search_leaf(leaf, batch).boxed_local(),
    })
    .try_flatten()
}

fn search_branch<K: Key, V: Value>(
    ipfs: IpfsService,
    branch: TreeNode<K, Branch>,
    codec: Codec,
    batch: impl IntoIterator<Item = K>,
) -> impl Stream<Item = Result<(K, V), Error>> {
    let batches = branch
        .search_batch(batch.into_iter())
        .map(|(link, batch)| Ok((ipfs.clone(), link, batch)))
        .collect::<Vec<_>>();

    stream::iter(batches.into_iter())
        .and_then(move |(ipfs, link, batch)| async move {
            match ipfs
                .dag_get::<&str, TreeNodes<K, V>>(link, None, codec)
                .await
            {
                Ok(node) => Ok((ipfs, node, batch)),
                Err(e) => Err(e),
            }
        })
        .map_ok(move |(ipfs, node, batch)| match node {
            TreeNodes::Branch(branch) => search_branch(ipfs, branch, codec, batch).boxed_local(),
            TreeNodes::Leaf(leaf) => search_leaf(leaf, batch).boxed_local(),
        })
        .try_flatten()
}

fn search_leaf<K: Key, V: Value>(
    mut leaf: TreeNode<K, Leaf<V>>,
    batch: Vec<K>,
) -> impl Stream<Item = Result<(K, V), Error>> {
    leaf.into_search_batch(batch.into_iter());

    let results: Vec<_> = leaf.into_iter().map(|item| Ok(item)).collect();

    stream::iter(results.into_iter())
}

/// Add or update values in the tree.
pub async fn batch_insert<K: Key, V: Value>(
    ipfs: IpfsService,
    root: Cid,
    config: Config,
    key_values: impl IntoIterator<Item = (K, V)>,
) -> Result<Cid, Error> {
    let mut batch = key_values.into_iter().collect::<Vec<_>>();

    batch.sort_unstable_by(|(a, _), (b, _)| a.cmp(b));
    batch.dedup_by(|(a, _), (b, _)| a == b);

    let mut key_links = execute_batch_insert(ipfs.clone(), root, config.clone(), batch).await?;

    while key_links.len() > 1 {
        let mut node = TreeNode::<K, Branch>::default();
        node.insert(key_links.into_iter());
        let nodes = node.split::<V>(config.clone())?;

        let nodes: Vec<TreeNodes<K, V>> = nodes
            .into_iter()
            .map(|branch| TreeNodes::Branch(branch))
            .collect();

        let keys = nodes
            .iter()
            .map(|node| match node {
                TreeNodes::Branch(node) => node.keys[0].clone(),
                TreeNodes::Leaf(node) => node.keys[0].clone(),
            })
            .collect::<Vec<_>>();

        let links = {
            let futures: Vec<_> = nodes
                .into_iter()
                .map(|node| {
                    let ipfs = ipfs.clone();

                    async move { ipfs.dag_put(&node, config.codec, config.codec).await }
                })
                .collect();

            try_join_all(futures).await?
        };

        key_links = keys.into_iter().zip(links.into_iter()).collect();
    }

    Ok(key_links[0].1)
}

#[async_recursion]
async fn execute_batch_insert<K: Key, V: Value>(
    ipfs: IpfsService,
    link: Cid,
    config: Config,
    batch: Vec<(K, V)>,
) -> Result<Vec<(K, Cid)>, Error> {
    let node = ipfs
        .dag_get::<&str, TreeNodes<K, V>>(link.into(), None, config.codec)
        .await?;

    let nodes: Vec<TreeNodes<K, V>> = match node {
        TreeNodes::Leaf(mut node) => {
            node.insert(batch.into_iter());

            let nodes = node.split(config.clone())?;

            nodes
                .into_iter()
                .map(|leaf| TreeNodes::Leaf(leaf))
                .collect()
        }
        TreeNodes::Branch(mut node) => {
            let futures: Vec<_> = node
                .insert_batch(batch)
                .map(|(link, batch)| {
                    execute_batch_insert(ipfs.clone(), link, config.clone(), batch)
                })
                .collect();

            let key_links = try_join_all(futures).await?;

            node.insert(key_links.into_iter().flatten());

            let nodes = node.split::<V>(config.clone())?;

            nodes
                .into_iter()
                .map(|branch| TreeNodes::Branch(branch))
                .collect()
        }
    };

    let keys = nodes
        .iter()
        .map(|node| match node {
            TreeNodes::Branch(node) => node.keys[0].clone(),
            TreeNodes::Leaf(node) => node.keys[0].clone(),
        })
        .collect::<Vec<_>>();

    let links = {
        let futures: Vec<_> = nodes
            .into_iter()
            .map(|node| {
                let ipfs = ipfs.clone();

                async move { ipfs.dag_put(&node, config.codec, config.codec).await }
            })
            .collect();

        try_join_all(futures).await?
    };

    let key_links = keys.into_iter().zip(links.into_iter()).collect();

    Ok(key_links)
}

/// Remove all values in the tree matching the keys.
pub async fn batch_remove<K: Key, V: Value>(
    ipfs: IpfsService,
    root: Cid,
    config: Config,
    keys: impl IntoIterator<Item = K>,
) -> Result<Cid, Error> {
    let mut batch = keys.into_iter().collect::<Vec<_>>();

    batch.sort_unstable();
    batch.dedup();

    let key_links =
        execute_batch_remove::<K, V>(ipfs.clone(), vec![root], config.clone(), batch).await?;

    if key_links.len() > 1 {
        let mut node = TreeNode::<K, Branch>::default();
        node.insert(key_links.into_iter());
        let node = TreeNodes::<K, V>::Branch(node);
        let cid = ipfs.dag_put(&node, config.codec, config.codec).await?;
        return Ok(cid);
    }

    if key_links.is_empty() {
        let node = TreeNode::<K, Leaf<V>>::default();
        let node = TreeNodes::Leaf(node);
        let root = ipfs.dag_put(&node, config.codec, config.codec).await?;
        return Ok(root);
    }

    Ok(key_links[0].1)
}

#[async_recursion]
async fn execute_batch_remove<K: Key, V: Value>(
    ipfs: IpfsService,
    links: Vec<Cid>,
    config: Config,
    batch: Vec<K>,
) -> Result<Vec<(K, Cid)>, Error> {
    let futures = links
        .into_iter()
        .map(|link| ipfs.dag_get::<&str, TreeNodes<K, V>>(link, None, config.codec))
        .collect::<Vec<_>>();

    let nodes = try_join_all(futures).await?;

    // Merge all the nodes
    // Works only because we know the nodes will be either leafs or branches.
    let node = nodes
        .into_iter()
        .reduce(|acc, x| match (acc, x) {
            (TreeNodes::Branch(mut node), TreeNodes::Branch(other)) => {
                node.merge(other);
                TreeNodes::Branch(node)
            }
            (TreeNodes::Leaf(mut node), TreeNodes::Leaf(other)) => {
                node.merge(other);
                TreeNodes::Leaf(node)
            }
            _ => panic!("The tree should always be symmetrical"),
        })
        .expect("at least one node");

    let nodes: Vec<_> = match node {
        TreeNodes::Leaf(mut node) => {
            node.remove_batch(batch.into_iter());

            let nodes = node.split(config.clone())?;

            nodes
                .into_iter()
                .map(|leaf| TreeNodes::Leaf(leaf))
                .collect()
        }
        TreeNodes::Branch(mut node) => {
            let futures: Vec<_> = node
                .remove_batch::<V>(batch)
                .map(|(links, batch)| {
                    execute_batch_remove::<K, V>(ipfs.clone(), links, config.clone(), batch)
                })
                .collect();

            let key_links = try_join_all(futures).await?;

            node.insert(key_links.into_iter().flatten());

            let nodes = node.split::<V>(config.clone())?;

            nodes
                .into_iter()
                .map(|branch| TreeNodes::Branch(branch))
                .collect()
        }
    };

    let keys = nodes
        .iter()
        .map(|node| match node {
            TreeNodes::Branch(node) => node.keys[0].clone(),
            TreeNodes::Leaf(node) => node.keys[0].clone(),
        })
        .collect::<Vec<_>>();

    let futures: Vec<_> = nodes
        .into_iter()
        .map(|node| {
            let ipfs = ipfs.clone();

            async move { ipfs.dag_put(&node, config.codec, config.codec).await }
        })
        .collect();

    let links = try_join_all(futures).await?;

    let key_links = keys.into_iter().zip(links.into_iter()).collect();

    Ok(key_links)
}

/// Stream all KVs in the tree in order.
pub fn stream_pairs<K: Key, V: Value>(
    ipfs: IpfsService,
    root: Cid,
    codec: Codec,
) -> impl Stream<Item = Result<(K, V), Error>> {
    stream::once(async move {
        match ipfs
            .dag_get::<&str, TreeNodes<K, V>>(root, None, codec)
            .await
        {
            Ok(node) => Ok((ipfs, node)),
            Err(e) => Err(e),
        }
    })
    .map_ok(move |(ipfs, node)| match node {
        TreeNodes::Branch(branch) => stream_branch(ipfs, branch, codec).boxed_local(),
        TreeNodes::Leaf(leaf) => stream::iter(leaf.into_iter().map(|item| Ok(item))).boxed_local(),
    })
    .try_flatten()
}

fn stream_branch<K: Key, V: Value>(
    ipfs: IpfsService,
    branch: TreeNode<K, Branch>,
    codec: Codec,
) -> impl Stream<Item = Result<(K, V), Error>> {
    stream::iter(branch.into_iter())
        .map(|(_, link)| Ok(link))
        .and_then(move |link| {
            let ipfs = ipfs.clone();

            async move {
                match ipfs
                    .dag_get::<&str, TreeNodes<K, V>>(link, None, codec)
                    .await
                {
                    Ok(node) => Ok((ipfs, node)),
                    Err(e) => Err(e),
                }
            }
        })
        .map_ok(move |(ipfs, node)| match node {
            TreeNodes::Branch(branch) => stream_branch(ipfs, branch, codec).boxed_local(),
            TreeNodes::Leaf(leaf) => stream::iter(leaf.into_iter())
                .map(|item| Ok(item))
                .boxed_local(),
        })
        .try_flatten()
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use crate::indexing::ordered_trees::prolly::{HashThreshold, Strategies};

    use super::*;

    use futures::StreamExt;

    use ipfs_api::IpfsService;

    use multihash::Multihash;

    use rand_xoshiro::{
        rand_core::{RngCore, SeedableRng},
        Xoshiro256StarStar,
    };

    use sha2::{Digest, Sha256};

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore]
    async fn tree_stream_all() {
        let mut rng = Xoshiro256StarStar::seed_from_u64(6784236783546783546u64);
        let ipfs = IpfsService::default();

        let config = Config::default();

        let node = TreeNode::<u32, Leaf<MockCID>>::default();
        let node = TreeNodes::Leaf(node);
        let root = ipfs
            .dag_put(&node, config.codec, config.codec)
            .await
            .expect("Root node");

        println!("Empty Root {}", root);

        let batch = unique_random_sorted_pairs(100_000, &mut rng);

        let tree_cid =
            batch_insert::<u32, MockCID>(ipfs.clone(), root, config.clone(), batch.clone())
                .await
                .expect("Batch insert");

        println!("New Root {}", tree_cid);

        let result: Vec<_> = stream_pairs::<u32, MockCID>(ipfs, tree_cid, config.codec)
            .collect()
            .await;
        let results: Result<Vec<_>, Error> = result.into_iter().collect();
        let result = results.expect("Tree Streaming");

        assert_eq!(result, batch);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore]
    async fn tree_remove_all() {
        let mut rng = Xoshiro256StarStar::seed_from_u64(6784236783546783546u64);
        let ipfs = IpfsService::default();

        let config = Config::default();

        let batch = unique_random_sorted_pairs(100_000, &mut rng);
        let (keys, _): (Vec<_>, Vec<_>) = batch.into_iter().unzip();

        //Run first test to generate the prolly tree
        let empty_tree_cid =
            Cid::try_from("bafyreiekfsw2g3fbdebzwf4equw2kygz6blz7uxtyvyd36xokibnd6hvgi").unwrap();
        let tree_cid =
            Cid::try_from("bafyreih2kps4md36dixdub2pha42b47iwvgybl2wb26tllg3332h5xo2dm").unwrap();

        let result = batch_remove::<u32, MockCID>(ipfs, tree_cid, config, keys)
            .await
            .expect("Empty tree");

        assert_eq!(result, empty_tree_cid);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore]
    async fn tree_batch_get() {
        let mut rng = Xoshiro256StarStar::seed_from_u64(6784236783546783546u64);
        let ipfs = IpfsService::default();

        let config = Config::default();

        //Run first test to generate the prolly tree
        let tree_cid =
            Cid::try_from("bafyreih2kps4md36dixdub2pha42b47iwvgybl2wb26tllg3332h5xo2dm").unwrap();

        let batch = unique_random_sorted_pairs(100_000, &mut rng);

        // the 6th key for each node
        let batch = vec![
            batch[6].clone(),
            batch[15960].clone(),
            batch[42846].clone(),
            batch[59698].clone(),
            batch[71612].clone(),
            batch[72209].clone(),
            batch[93232].clone(),
        ];

        let keys: Vec<_> = batch.clone().into_iter().map(|(key, _)| key).collect();

        let result: Vec<_> = batch_get::<u32, MockCID>(ipfs, tree_cid, config.codec, keys)
            .collect()
            .await;
        let results: Result<Vec<_>, Error> = result.into_iter().collect();
        let result = results.expect("Tree Batch Get");

        assert_eq!(result, batch);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore]
    async fn tree_insert_remove_bound() {
        let mut config = Config::default();

        let mut rng = Xoshiro256StarStar::seed_from_u64(6784236783546783546u64);
        rng.jump();

        let mut batch = Vec::new();

        loop {
            let key = rng.next_u32();
            let mut value = [0u8; 32];
            rng.fill_bytes(&mut value);
            let value = value.to_vec();

            let bound = config.boundary(key, value.clone()).unwrap();

            if bound {
                //println!("Bound Key {}, Value {:?}", key, value);
                batch.push((key, value));
                break;
            }
        }

        let ipfs = IpfsService::default();

        let config = Config::default();

        //Run first test to generate the prolly tree
        let tree_cid =
            Cid::try_from("bafyreih2kps4md36dixdub2pha42b47iwvgybl2wb26tllg3332h5xo2dm").unwrap();

        let root =
            batch_insert::<u32, MockCID>(ipfs.clone(), tree_cid, config.clone(), batch.clone())
                .await
                .expect("Full tree");

        println!("New Root {}", root);

        let (keys, _): (Vec<_>, Vec<_>) = batch.clone().into_iter().unzip();

        let result = batch_remove::<u32, MockCID>(ipfs, root, config, keys)
            .await
            .unwrap();

        assert_eq!(result, tree_cid);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore]
    async fn tree_tall() {
        let mut rng = Xoshiro256StarStar::seed_from_u64(7835467835467354678u64);
        let ipfs = IpfsService::default();

        let mut config = Config::default();
        let mut strat = HashThreshold::default();
        strat.chunking_factor = 1 << 24;
        config.chunking_strategy = Strategies::Threshold(strat);

        let node = TreeNode::<u32, Leaf<MockCID>>::default();
        let node = TreeNodes::Leaf(node);
        let root = ipfs
            .dag_put(&node, config.codec, config.codec)
            .await
            .expect("Root node");

        println!("Empty Root {}", root);

        let batch = unique_random_sorted_pairs(1_000_000, &mut rng);

        let tree_cid =
            batch_insert::<u32, MockCID>(ipfs.clone(), root, config.clone(), batch.clone())
                .await
                .expect("Batch insert");

        println!("New Root {}", tree_cid);
    }

    fn _random_cid(rng: &mut Xoshiro256StarStar) -> Cid {
        let mut input = [0u8; 64];
        rng.fill_bytes(&mut input);

        let hash = Sha256::new_with_prefix(input).finalize();

        let multihash = Multihash::wrap(0x13, &hash).unwrap();

        Cid::new_v1(/* DAG-CBOR */ 0x71, multihash)
    }

    type MockCID = Vec<u8>;

    fn unique_random_sorted_pairs(
        numb: usize,
        rng: &mut Xoshiro256StarStar,
    ) -> Vec<(u32, MockCID)> {
        let mut key_values = Vec::with_capacity(numb);

        for _ in 0..numb {
            let key = rng.next_u32();
            let mut value = [0u8; 32];
            rng.fill_bytes(&mut value);

            key_values.push((key, value.to_vec()));
        }

        key_values.sort_unstable_by(|(a, _), (b, _)| a.cmp(&b));
        key_values.dedup_by(|(a, _), (b, _)| a == b);

        key_values
    }

    fn _unique_random_sorted_batch(numb: usize, rng: &mut Xoshiro256StarStar) -> VecDeque<u64> {
        let mut keys = Vec::with_capacity(numb);

        for _ in 0..numb {
            let key = rng.next_u64();

            keys.push(key);
        }

        keys.sort_unstable();
        keys.dedup();

        keys.into()
    }
}

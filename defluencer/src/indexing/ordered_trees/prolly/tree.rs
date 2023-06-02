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

    use crate::indexing::ordered_trees::prolly::{HashThreshold, Strategies};

    use super::*;

    use futures::StreamExt;

    use ipfs_api::IpfsService;

    use rand::prelude::*;

    use rand_xoshiro::Xoshiro256StarStar;

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn tree_stream_all() {
        let mut rng = Xoshiro256StarStar::seed_from_u64(6784236783546783546u64);
        let ipfs = IpfsService::default();

        let mut config = Config::default();
        let mut strat = HashThreshold::default();
        strat.chunking_factor = 1 << 19;
        config.chunking_strategy = Strategies::Threshold(strat);

        let node = TreeNode::<u16, Leaf<DataBlob>>::default();
        let node = TreeNodes::Leaf(node);
        let root = ipfs
            .dag_put(&node, config.codec, config.codec)
            .await
            .expect("Root node");

        println!("Empty Root {}", root);

        let batch = unique_random_sorted_pairs::<32>(10_000, &mut rng);

        let tree_cid =
            batch_insert::<u16, DataBlob>(ipfs.clone(), root, config.clone(), batch.clone())
                .await
                .expect("Batch insert");

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
    async fn tree_batch_insert() {
        let mut rng = Xoshiro256StarStar::seed_from_u64(6784236783546783546u64);
        let ipfs = IpfsService::default();

        let mut config = Config::default();
        let mut strat = HashThreshold::default();
        strat.chunking_factor = 1 << 19;
        config.chunking_strategy = Strategies::Threshold(strat);

        let original_batch = unique_random_sorted_pairs::<32>(10_000, &mut rng);

        //Run first test to generate the prolly tree
        let tree_cid =
            Cid::try_from("bafyreiacttehgexdhblgzfcco2chzf64s6x3e6asyzhyr4qhh2vmwkaiwu").unwrap();

        let mut rng = Xoshiro256StarStar::from_entropy();

        let batch = unique_random_sorted_pairs::<32>(100, &mut rng);

        /* println!(
            "Test Insert Keys {:?}",
            batch.iter().map(|(key, _)| *key).collect::<Vec<_>>()
        ); */

        let tree_cid = batch_insert::<u16, DataBlob>(
            ipfs.clone(),
            tree_cid,
            config.clone(),
            batch.clone().into_iter(),
        )
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
    async fn tree_batch_remove() {
        let mut rng = Xoshiro256StarStar::seed_from_u64(6784236783546783546u64);
        let ipfs = IpfsService::default();

        let mut config = Config::default();
        let mut strat = HashThreshold::default();
        strat.chunking_factor = 1 << 19;
        config.chunking_strategy = Strategies::Threshold(strat);

        let mut batch = unique_random_sorted_pairs::<32>(10_000, &mut rng);

        //Run first test to generate the prolly tree
        let tree_cid =
            Cid::try_from("bafyreiacttehgexdhblgzfcco2chzf64s6x3e6asyzhyr4qhh2vmwkaiwu").unwrap();

        let mut rng = Xoshiro256StarStar::from_entropy();

        // 100 random KVs
        let mut keys = Vec::with_capacity(10);

        for _ in 0..100 {
            let (key, _) = batch.remove(rng.gen_range(0..batch.len()));
            keys.push(key);
        }

        keys.sort_unstable();
        keys.dedup();

        //println!("Test Remove Keys {:?}", keys);

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

        let result_keys: Vec<_> = result.into_iter().map(|(key, _)| key).collect();
        let batch_keys: Vec<_> = batch.into_iter().map(|(key, _)| key).collect();

        assert_eq!(result_keys, batch_keys);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn tree_remove_all() {
        let mut rng = Xoshiro256StarStar::seed_from_u64(6784236783546783546u64);
        let ipfs = IpfsService::default();

        let mut config = Config::default();
        let mut strat = HashThreshold::default();
        strat.chunking_factor = 1 << 19;
        config.chunking_strategy = Strategies::Threshold(strat);

        let batch = unique_random_sorted_pairs::<32>(10_000, &mut rng);
        let keys: Vec<_> = batch.into_iter().map(|(key, _)| key).collect();

        //Run first test to generate the prolly tree
        let empty_tree_cid =
            Cid::try_from("bafyreiekfsw2g3fbdebzwf4equw2kygz6blz7uxtyvyd36xokibnd6hvgi").unwrap();
        let tree_cid =
            Cid::try_from("bafyreiacttehgexdhblgzfcco2chzf64s6x3e6asyzhyr4qhh2vmwkaiwu").unwrap();

        let result = batch_remove::<u16, DataBlob>(ipfs, tree_cid, config, keys)
            .await
            .expect("Empty tree");

        assert_eq!(result, empty_tree_cid);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn tree_fuzz() {
        let mut rng = Xoshiro256StarStar::seed_from_u64(7835467835467354678u64);
        let ipfs = IpfsService::default();

        let mut config = Config::default();
        let mut strat = HashThreshold::default();
        strat.chunking_factor = 1 << 20;
        config.chunking_strategy = Strategies::Threshold(strat);

        let node = TreeNode::<u32, Leaf<DataBlob>>::default();
        let node = TreeNodes::Leaf(node);
        let mut root = ipfs
            .dag_put(&node, config.codec, config.codec)
            .await
            .expect("Root node");

        let mut added = vec![];

        for _ in 0..1000 {
            let add = rng.gen_bool(2.0 / 3.0);

            if add {
                let numb = rng.gen_range(1..15);
                let batch = unique_random_sorted_pairs::<100_000>(numb, &mut rng);

                root = batch_insert::<u16, DataBlob>(
                    ipfs.clone(),
                    root,
                    config.clone(),
                    batch.clone(),
                )
                .await
                .expect("Batch insert");

                added.extend(batch.into_iter());
            } else {
                if added.is_empty() {
                    continue;
                }

                let mut batch = vec![];

                let numb = rng.gen_range(1..15);

                for _ in 0..numb {
                    if added.is_empty() {
                        continue;
                    }

                    let idx = rng.gen_range(0..added.len());
                    let (key, _) = added.swap_remove(idx);
                    batch.push(key);
                }

                batch.sort_unstable();
                batch.dedup();

                root = batch_remove::<u16, DataBlob>(ipfs.clone(), root, config.clone(), batch)
                    .await
                    .expect("Batch remove");
            }
        }
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

use async_recursion::async_recursion;

use futures::{future::try_join_all, stream, Stream, StreamExt, TryStreamExt};

use ipfs_api::IpfsService;

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
    keys: impl IntoIterator<Item = K>,
) -> impl Stream<Item = Result<(K, V), Error>> {
    let mut batch = keys.into_iter().collect::<Vec<_>>();
    batch.sort_unstable();

    stream::once(async move {
        match ipfs.dag_get::<&str, TreeNodes<K, V>>(root, None).await {
            Ok(node) => Ok((ipfs, node, batch)),
            Err(e) => Err(e),
        }
    })
    .map_ok(|(ipfs, node, batch)| match node {
        TreeNodes::Branch(branch) => search_branch(ipfs, branch, batch).boxed_local(),
        TreeNodes::Leaf(leaf) => search_leaf(leaf, batch).boxed_local(),
    })
    .try_flatten()
}

fn search_branch<K: Key, V: Value>(
    ipfs: IpfsService,
    branch: TreeNode<K, Branch>,
    batch: impl IntoIterator<Item = K>,
) -> impl Stream<Item = Result<(K, V), Error>> {
    let batches = branch
        .search_batch(batch.into_iter())
        .map(|(link, batch)| Ok((ipfs.clone(), link, batch)))
        .collect::<Vec<_>>();

    stream::iter(batches.into_iter())
        .and_then(|(ipfs, link, batch)| async move {
            match ipfs.dag_get::<&str, TreeNodes<K, V>>(link, None).await {
                Ok(node) => Ok((ipfs, node, batch)),
                Err(e) => Err(e),
            }
        })
        .map_ok(|(ipfs, node, batch)| match node {
            TreeNodes::Branch(branch) => search_branch(ipfs, branch, batch).boxed_local(),
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

    let key_links = execute_batch_insert(ipfs.clone(), root, config.clone(), batch).await?;

    if key_links.len() > 1 {
        let mut node = TreeNode::<K, Branch>::default();

        node.insert(key_links.into_iter());

        let node = TreeNodes::<K, V>::Branch(node);

        let cid = ipfs.dag_put(&node, config.codec).await?;

        return Ok(cid);
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
        .dag_get::<&str, TreeNodes<K, V>>(link.into(), None)
        .await?;

    let nodes: Vec<TreeNodes<K, V>> = match node {
        TreeNodes::Leaf(mut node) => {
            node.insert(batch.into_iter());

            let nodes = node.split_with(config.clone())?;

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

            let nodes = node.split_with::<V>(config.clone())?;

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

                async move { ipfs.dag_put(&node, config.codec).await }
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

    let key_links =
        execute_batch_remove::<K, V>(ipfs.clone(), vec![root], config.clone(), batch).await?;

    if key_links.len() > 1 {
        let mut node = TreeNode::<K, Branch>::default();
        node.insert(key_links.into_iter());
        let node = TreeNodes::<K, V>::Branch(node);

        let cid = ipfs.dag_put(&node, config.codec).await?;

        return Ok(cid);
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
        .map(|link| ipfs.dag_get::<&str, TreeNodes<K, V>>(link, None))
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

            let nodes = node.split_with(config.clone())?;

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

            let nodes = node.split_with::<V>(config.clone())?;

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

            async move { ipfs.dag_put(&node, config.codec).await }
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
) -> impl Stream<Item = Result<(K, V), Error>> {
    stream::once(async move {
        match ipfs.dag_get::<&str, TreeNodes<K, V>>(root, None).await {
            Ok(node) => Ok((ipfs, node)),
            Err(e) => Err(e),
        }
    })
    .map_ok(move |(ipfs, node)| match node {
        TreeNodes::Branch(branch) => stream_branch(ipfs, branch).boxed_local(),
        TreeNodes::Leaf(leaf) => stream::iter(leaf.into_iter().map(|item| Ok(item))).boxed_local(),
    })
    .try_flatten()
}

fn stream_branch<K: Key, V: Value>(
    ipfs: IpfsService,
    branch: TreeNode<K, Branch>,
) -> impl Stream<Item = Result<(K, V), Error>> {
    stream::iter(branch.into_iter())
        .map(|(_, link)| Ok(link))
        .and_then(move |link| {
            let ipfs = ipfs.clone();

            async move {
                match ipfs.dag_get::<&str, TreeNodes<K, V>>(link, None).await {
                    Ok(node) => Ok((ipfs, node)),
                    Err(e) => Err(e),
                }
            }
        })
        .map_ok(move |(ipfs, node)| match node {
            TreeNodes::Branch(branch) => stream_branch(ipfs, branch).boxed_local(),
            TreeNodes::Leaf(leaf) => stream::iter(leaf.into_iter())
                .map(|item| Ok(item))
                .boxed_local(),
        })
        .try_flatten()
}

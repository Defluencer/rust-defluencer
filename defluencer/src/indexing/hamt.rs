use std::collections::BTreeSet;

use arrayvec::ArrayVec;

use async_recursion::async_recursion;

use cid::Cid;

use futures::{stream, Stream, StreamExt, TryStreamExt};

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::{
    indexes::hamt::{
        BitField, BucketEntry, Element, HAMTNode, HAMTRoot, BUCKET_SIZE, DIGEST_LENGTH_BYTES,
        HASH_ALGORITHM,
    },
    IPLDLink,
};

use crate::errors::Error;

#[derive(thiserror::Error, Debug)]
pub enum HAMTError {
    #[error("Max depth reached")]
    MaxDepth,

    #[error("Wrong hash algorithm")]
    HashAlgo,

    #[error("Cannot remove key, not present")]
    RemoveFailed,
}

pub async fn get(ipfs: &IpfsService, root: IPLDLink, key: Cid) -> Result<Option<Cid>, Error> {
    if key.hash().code() != HASH_ALGORITHM as u64 {
        return Err(HAMTError::HashAlgo.into());
    }

    let key: ArrayVec<u8, DIGEST_LENGTH_BYTES> =
        key.hash().digest().iter().map(|byte| *byte).collect();
    let key = key.into_inner().unwrap();

    let root = ipfs.dag_get::<&str, HAMTRoot>(root.link, None).await?;

    let mut depth = 0;
    let mut node = root.hamt;

    loop {
        let index = key[depth] as usize;
        let map = BitField::from(node.map);
        let data_index = map[0..index].count_ones();

        if !map[index] {
            // CASE: index bit is not set
            return Ok(None);
        }

        // CASE: index bit is set
        match &node.data[data_index] {
            Element::Link(ipld) => {
                if (depth + 1) > DIGEST_LENGTH_BYTES {
                    return Err(HAMTError::MaxDepth.into());
                }

                node = ipfs.dag_get::<&str, HAMTNode>(ipld.link, None).await?;
                depth += 1;

                continue;
            }
            Element::Bucket(btree) => {
                let entry = BucketEntry {
                    key,
                    value: Default::default(),
                };

                match btree.get(&entry) {
                    Some(entry) => return Ok(Some(entry.value.link)),
                    None => return Ok(None),
                }
            }
        }
    }
}

pub async fn insert(
    ipfs: &IpfsService,
    index_cid: IPLDLink,
    key: Cid,
    value: Cid,
) -> Result<Cid, Error> {
    if key.hash().code() != HASH_ALGORITHM as u64 {
        return Err(HAMTError::HashAlgo.into());
    }

    let key: ArrayVec<u8, DIGEST_LENGTH_BYTES> =
        key.hash().digest().iter().map(|byte| *byte).collect();
    let key = key.into_inner().unwrap();

    let mut root = ipfs.dag_get::<&str, HAMTRoot>(index_cid.link, None).await?;

    set(ipfs, key, value.into(), 0, &mut root.hamt).await?;

    let cid = ipfs.dag_put(&root, Codec::default()).await?;

    Ok(cid)
}

#[async_recursion(?Send)]
async fn set(
    ipfs: &IpfsService,
    key: [u8; DIGEST_LENGTH_BYTES],
    value: IPLDLink,
    depth: usize,
    node: &mut HAMTNode,
) -> Result<Cid, Error> {
    let index = key[depth] as usize;
    let mut map = BitField::from(node.map);
    let data_index = map[0..index].count_ones();

    if !map[index] {
        // CASE: index bit is not set

        let entry = BucketEntry { key, value };
        let bucket = Element::Bucket(BTreeSet::from([entry]));

        node.data.insert(data_index, bucket);

        map.set(index, true);
        node.map = map.into_inner();

        let cid = ipfs.dag_put(&node, Codec::default()).await?;

        return Ok(cid);
    }

    // CASE: index bit is set

    match &mut node.data[data_index] {
        Element::Link(ipld) => {
            if (depth + 1) > DIGEST_LENGTH_BYTES {
                return Err(HAMTError::MaxDepth.into());
            }

            let mut new_node = ipfs.dag_get::<&str, HAMTNode>(ipld.link, None).await?;

            let cid = set(ipfs, key, value, depth + 1, &mut new_node).await?;

            *ipld = cid.into();

            let cid = ipfs.dag_put(&node, Codec::default()).await?;

            return Ok(cid);
        }
        Element::Bucket(btree) => {
            if btree.len() < BUCKET_SIZE {
                let entry = BucketEntry { key, value };

                btree.insert(entry);

                let cid = ipfs.dag_put(&node, Codec::default()).await?;

                return Ok(cid);
            }

            let mut new_node = HAMTNode::default();

            for item in btree.iter() {
                set(ipfs, item.key, item.value, depth + 1, &mut new_node).await?;
            }

            let cid = set(ipfs, key, value, depth + 1, &mut new_node).await?;

            node.data[data_index] = Element::Link(cid.into());

            let cid = ipfs.dag_put(&node, Codec::default()).await?;

            return Ok(cid);
        }
    }
}

pub async fn remove(ipfs: &IpfsService, index: IPLDLink, key: Cid) -> Result<Cid, Error> {
    if key.hash().code() != HASH_ALGORITHM as u64 {
        return Err(HAMTError::HashAlgo.into());
    }

    let key: ArrayVec<u8, DIGEST_LENGTH_BYTES> =
        key.hash().digest().iter().map(|byte| *byte).collect();
    let key = key.into_inner().unwrap();

    let mut root = ipfs.dag_get::<&str, HAMTRoot>(index.link, None).await?;

    delete(ipfs, key, 0, &mut root.hamt).await?;

    let cid = ipfs.dag_put(&root, Codec::default()).await?;

    Ok(cid)
}

#[async_recursion(?Send)]
async fn delete(
    ipfs: &IpfsService,
    key: [u8; DIGEST_LENGTH_BYTES],
    depth: usize,
    node: &mut HAMTNode,
) -> Result<Element, Error> {
    let index = key[depth] as usize;
    let mut map = BitField::from(node.map);
    let data_index = map[0..index].count_ones();

    /* println!(
        "Index: {} Depth: {} Data Index: {} Index Bit: {}",
        index, depth, data_index, map[index]
    ); */

    if !map[index] {
        return Err(HAMTError::RemoveFailed.into());
    }

    if let Element::Link(ipld) = node.data[data_index] {
        //println!("Found Link");

        if (depth + 1) > DIGEST_LENGTH_BYTES {
            return Err(HAMTError::MaxDepth.into());
        }

        let mut new_node = ipfs.dag_get::<&str, HAMTNode>(ipld.link, None).await?;

        let element = delete(ipfs, key, depth + 1, &mut new_node).await?;

        node.data[data_index] = element;

        if let Element::Link(_) = node.data[data_index] {
            let cid = ipfs.dag_put(&node, Codec::default()).await?;

            return Ok(Element::Link(cid.into()));
        }
    }

    //println!("Found Bucket");

    let (links, entrees) =
        node.data
            .iter()
            .fold((0usize, 0usize), |(mut links, mut entrees), element| {
                match element {
                    Element::Link(_) => {
                        links += 1;
                    }
                    Element::Bucket(vec) => {
                        entrees += vec.len();
                    }
                }

                (links, entrees)
            });

    if depth == 0 || links > 0 || entrees > (BUCKET_SIZE + 1) {
        //println!("Not collapsing bucket into parent");

        if let Element::Bucket(btree) = &mut node.data[data_index] {
            if btree.len() > 1 {
                let entry = BucketEntry {
                    key,
                    value: Default::default(),
                };

                if btree.remove(&entry) {
                    //println!("Entry removed");
                }
            } else {
                map.set(index, false);
                node.map = map.into_inner();

                node.data.remove(data_index);

                //println!("Bit unset & Data removed");
            }

            let cid = ipfs.dag_put(&node, Codec::default()).await?;

            return Ok(Element::Link(cid.into()));
        }
    }

    //println!("Collapsing bucket into parent");

    let mut btree: BTreeSet<BucketEntry> = node
        .data
        .iter()
        .filter_map(|element| {
            if let Element::Bucket(btree) = element {
                Some(btree.into_iter())
            } else {
                None
            }
        })
        .flatten()
        .map(|bucket| *bucket)
        .collect();

    let entry = BucketEntry {
        key,
        value: Default::default(),
    };

    if btree.remove(&entry) {
        //println!("Entry removed");
    }

    Ok(Element::Bucket(btree))
}

pub fn values(ipfs: &IpfsService, root: IPLDLink) -> impl Stream<Item = Result<Cid, Error>> + '_ {
    stream::try_unfold(Some(root), move |mut root| async move {
        let ipld = match root.take() {
            Some(ipld) => ipld,
            None => return Result::<_, Error>::Ok(None),
        };

        let root_node = ipfs.dag_get::<&str, HAMTRoot>(ipld.link, None).await?;

        let stream = stream_data(ipfs, root_node.hamt);

        Ok(Some((stream, root)))
    })
    .try_flatten()
}

fn stream_data(ipfs: &IpfsService, node: HAMTNode) -> impl Stream<Item = Result<Cid, Error>> + '_ {
    stream::try_unfold(node.data.into_iter(), move |mut iter| async move {
        let element = match iter.next() {
            Some(element) => element,
            None => return Result::<_, Error>::Ok(None),
        };

        match element {
            Element::Link(ipld) => {
                let node = ipfs.dag_get::<&str, HAMTNode>(ipld.link, None).await?;

                let stream = stream_data(ipfs, node).boxed_local();

                Ok(Some((stream, iter)))
            }
            Element::Bucket(vec) => {
                let stream =
                    stream::iter(vec.into_iter().map(|entry| Ok(entry.value.link))).boxed_local();

                Ok(Some((stream, iter)))
            }
        }
    })
    .try_flatten()
}

/* pub async fn values(ipfs: &IpfsService, root: Cid) -> Result<Vec<Cid>, Error> {
    let root = ipfs.dag_get::<&str, HAMTRoot>(root, None).await?;

    get_values(ipfs, root.hamt).await
}

#[async_recursion(?Send)]
async fn get_values(ipfs: &IpfsService, node: HAMTNode) -> Result<Vec<Cid>, Error> {
    let mut values = Vec::with_capacity(node.data.len());

    for element in node.data {
        match element {
            Element::Link(ipld) => {
                let node = ipfs.dag_get::<&str, HAMTNode>(ipld.link, None).await?;

                let result = get_values(ipfs, node).await?;

                values.extend(result);
            }
            Element::Bucket(vec) => values.extend(vec.into_iter().map(|entry| entry.value.link)),
        }
    }

    Ok(values)
} */

/* pub fn values(ipfs: &IpfsService, root: IPLDLink) -> impl Stream<Item = Cid> + '_ {
    stream::unfold(Some(root), move |mut root| async move {
        match root {
            Some(ipld) => match ipfs.dag_get::<&str, HAMTRoot>(ipld.link, None).await {
                Ok(root_node) => {
                    root = None;

                    let stream = stream_data(ipfs, root_node.hamt);

                    Some((stream, root))
                }
                Err(_) => None,
            },
            None => None,
        }
    })
    .flatten()
} */

/* fn stream_data(ipfs: &IpfsService, node: HAMTNode) -> impl Stream<Item = Cid> + '_ {
    stream::unfold(node.data.into_iter(), move |mut iter| async move {
        match iter.next() {
            Some(element) => match element {
                Element::Link(ipld) => {
                    match ipfs.dag_get::<&str, HAMTNode>(ipld.link, None).await {
                        Ok(node) => {
                            let stream = stream_data(ipfs, node).boxed_local();

                            Some((stream, iter))
                        }
                        Err(_) => None,
                    }
                }
                Element::Bucket(vec) => {
                    let stream =
                        stream::iter(vec.into_iter().map(|entry| entry.value.link)).boxed_local();

                    Some((stream, iter))
                }
            },
            None => None,
        }
    })
    .flatten()
} */

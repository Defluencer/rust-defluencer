use std::collections::VecDeque;

use crate::errors::Error;

use arrayvec::ArrayVec;

use async_recursion::async_recursion;

use cid::Cid;

use either::Either;
use futures::{
    stream::{self, FuturesUnordered},
    Stream, StreamExt, TryStreamExt,
};

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::{
    indexes::hamt::{
        BitField, BucketEntry, Element, HAMTNode, HAMTRoot, BUCKET_SIZE, DIGEST_LENGTH_BYTES,
    },
    IPLDLink,
};

pub async fn get(ipfs: &IpfsService, root: IPLDLink, key: Cid) -> Result<Cid, Error> {
    if key.hash().size() != DIGEST_LENGTH_BYTES as u8 {
        return Err(Error::NotFound); //TODO add error type
    }

    let key: ArrayVec<u8, DIGEST_LENGTH_BYTES> = key.hash().to_bytes().into_iter().collect();
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
            return Err(Error::NotFound);
        }

        // CASE: index bit is set
        match &node.data[data_index] {
            Element::Link(ipld) => {
                if (depth + 1) > DIGEST_LENGTH_BYTES {
                    // MAX Collisions Error
                    return Err(Error::NotFound); // TODO add new error
                }

                node = ipfs.dag_get::<&str, HAMTNode>(ipld.link, None).await?;
                depth += 1;

                continue;
            }
            Element::Bucket(vec) => {
                let entry = BucketEntry {
                    key,
                    value: Default::default(),
                };

                match vec.binary_search(&entry) {
                    Ok(idx) => return Ok(vec[idx].value.link),
                    Err(_) => return Err(Error::NotFound),
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
    if key.hash().size() != DIGEST_LENGTH_BYTES as u8 {
        return Err(Error::NotFound); //TODO add error type
    }

    let key: ArrayVec<u8, DIGEST_LENGTH_BYTES> = key.hash().to_bytes().into_iter().collect();
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

    if map[index] {
        // CASE: index bit is not set

        let entry = BucketEntry { key, value };
        let bucket = Element::Bucket(vec![entry]);

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
                // MAX Collisions Error
                return Err(Error::NotFound); // TODO add new error
            }

            let mut new_node = ipfs.dag_get::<&str, HAMTNode>(ipld.link, None).await?;

            let cid = set(ipfs, key, value, depth + 1, &mut new_node).await?;

            *ipld = cid.into();

            let cid = ipfs.dag_put(&node, Codec::default()).await?;

            return Ok(cid);
        }
        Element::Bucket(vec) => {
            if vec.len() < BUCKET_SIZE {
                let entry = BucketEntry { key, value };

                let idx = vec.binary_search(&entry).unwrap_or_else(|x| x);
                vec.insert(idx, entry);

                let cid = ipfs.dag_put(&node, Codec::default()).await?;

                return Ok(cid);
            }

            let mut new_node = HAMTNode::default();

            for item in vec.iter() {
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
    if key.hash().size() != DIGEST_LENGTH_BYTES as u8 {
        return Err(Error::NotFound); //TODO add error type
    }

    let key: ArrayVec<u8, DIGEST_LENGTH_BYTES> = key.hash().to_bytes().into_iter().collect();
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

    if !map[index] {
        // CASE: index bit is not set
        return Err(Error::NotFound);
    }

    // CASE: index bit is set

    if let Element::Link(ipld) = node.data[data_index] {
        if (depth + 1) > DIGEST_LENGTH_BYTES {
            // MAX Collisions Error
            return Err(Error::NotFound); // TODO add new error type
        }

        let mut new_node = ipfs.dag_get::<&str, HAMTNode>(ipld.link, None).await?;

        let element = delete(ipfs, key, depth + 1, &mut new_node).await?;

        node.data[data_index] = element;

        if let Element::Link(_) = node.data[data_index] {
            let cid = ipfs.dag_put(&node, Codec::default()).await?;

            return Ok(Element::Link(cid.into()));
        }
    }

    let mut links = 0;
    let mut entrees = 0;

    for element in node.data.iter() {
        match element {
            Element::Link(_) => {
                links += 1;
            }
            Element::Bucket(vec) => {
                entrees += vec.len();
            }
        }
    }

    if depth == 0 || links > 0 || entrees > (BUCKET_SIZE + 1) {
        if let Element::Bucket(vec) = &mut node.data[data_index] {
            if vec.len() > 1 {
                let entry = BucketEntry {
                    key,
                    value: Default::default(),
                };

                match vec.binary_search(&entry) {
                    Ok(idx) => {
                        vec.remove(idx);
                    }
                    Err(_) => return Err(Error::NotFound),
                }
            } else {
                node.data.remove(data_index);

                map.set(index, false);
                node.map = map.into_inner();
            }

            let cid = ipfs.dag_put(&node, Codec::default()).await?;

            return Ok(Element::Link(cid.into()));
        }
    }

    if depth != 0 && links == 0 && entrees == (BUCKET_SIZE + 1) {
        // Collapse node into parent
        if let Element::Bucket(vec) = &mut node.data[data_index] {
            let entry = BucketEntry {
                key,
                value: Default::default(),
            };

            match vec.binary_search(&entry) {
                Ok(idx) => {
                    vec.remove(idx);
                }
                Err(_) => return Err(Error::NotFound),
            }

            return Ok(Element::Bucket(vec.clone()));
        }
    }

    Err(Error::NotFound)
}

/* pub async fn values(ipfs: &IpfsService, root: Cid) -> Result<Vec<Vec<BucketEntry>>, Error> {

    stream::unfold(Some(root), move |mut root| async move {
        match root {
            Some(cid) => match ipfs.dag_get::<&str, HAMTRoot>(cid, None).await {
                Ok(root_node) => {
                    root = None;

                    Some((root_node.hamt, root))
                }
                Err(_) => None,
            },
            None => None,
        }
    })
    .flat_map(|node| stream_node_data(ipfs, node))
    .flat_map(|element| stream_elements(ipfs, element))
} */

/* fn stream_node_data(ipfs: &IpfsService, node: HAMTNode) -> impl Stream<Item = Element> + '_ {
    stream::unfold((0, node.data), move |(mut idx, mut data)| async move {
        let element = data[idx];

        match element {
            Element::Link(ipld) => match ipfs.dag_get::<&str, HAMTNode>(ipld.link, None).await {
                Ok(node) => {
                    let end = idx + node.data.len();
                    data.splice(idx..end, node.data);

                    Some((element, (idx, data)))
                }
                Err(_) => None,
            },
            Element::Bucket(vec) => {
                data.push(element);
                idx += 1;

                Some((element, (idx, data)))
            }
        }
    })
} */

/* fn stream_elements(ipfs: &IpfsService, element: Element) -> impl Stream<Item = Cid> + '_ {
    stream::unfold(Some(element), move |mut element| async move {
        match element {
            Some(item) => match item {
                Element::Link(ipld) => {
                    match ipfs.dag_get::<&str, HAMTNode>(ipld.link, None).await {
                        Ok(node) => {
                            element = None;

                            let stream = stream_node_data(ipfs, node)
                                .flat_map(|element| stream_elements(ipfs, element));

                            Some((stream, element))
                        }
                        Err(_) => todo!(),
                    }
                }
                Element::Bucket(vec) => {
                    element = None;

                    let stream = stream::iter(vec.into_iter().map(|entry| entry.value.link));

                    Some((stream, element))
                }
            },
            None => None,
        }
    })
    .flatten()
} */

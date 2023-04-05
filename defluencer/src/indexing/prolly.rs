use std::{
    collections::{hash_map::DefaultHasher, VecDeque},
    fmt::Debug,
    hash::{Hash, Hasher},
    ops::{Bound, RangeBounds},
    vec,
};

use async_recursion::async_recursion;

use futures::{
    channel::mpsc::{self, Sender},
    future::join_all,
    stream::{self, FuturesUnordered},
    FutureExt, Stream, StreamExt, TryStreamExt,
};

use either::Either::{self, Right};

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::types::IPLDLink;

use num::{BigUint, Integer, Zero};

use crate::errors::Error;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use sha2::{Digest, Sha512};

pub trait Key:
    Default
    + Debug
    + Clone
    + Copy
    + Eq
    + Ord
    + Hash
    + Serialize
    + DeserializeOwned
    + Send
    + Sync
    + Sized
    + 'static
{
}
impl<
        T: Default
            + Debug
            + Clone
            + Copy
            + Eq
            + Ord
            + Hash
            + Serialize
            + DeserializeOwned
            + Send
            + Sync
            + Sized
            + 'static,
    > Key for T
{
}

pub trait Value:
    Default + Debug + Clone + Copy + Eq + Serialize + DeserializeOwned + Send + Sync + Sized + 'static
{
}
impl<
        T: Default
            + Debug
            + Clone
            + Copy
            + Eq
            + Serialize
            + DeserializeOwned
            + Send
            + Sync
            + Sized
            + 'static,
    > Value for T
{
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct TreeNode<K, V> {}

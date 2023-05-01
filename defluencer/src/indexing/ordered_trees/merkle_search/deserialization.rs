use std::collections::VecDeque;

use super::{
    config::{Config, Tree},
    node::TreeNode,
};

use crate::indexing::ordered_trees::{
    errors::Error,
    traits::{Key, Value},
};

use ipfs_api::responses::Codec;

use libipld_core::ipld::Ipld;

use multihash::Code;

use serde_ipld_dagcbor::DecodeError;

use num::FromPrimitive;

// Can link indexes be omited for a more compact representation?

impl<K: Key, V: Value> TryFrom<Ipld> for TreeNode<K, V> {
    type Error = Error;

    fn try_from(ipld: Ipld) -> Result<Self, Self::Error> {
        let mut list: Vec<Ipld> = ipld.try_into()?;

        if list.len() != 5 {
            return Err(DecodeError::RequireLength {
                name: "tuple",
                expect: 5,
                value: list.len(),
            }
            .into());
        };

        let links: Vec<Ipld> = list.pop().unwrap().try_into()?;
        let indexes: Vec<Ipld> = list.pop().unwrap().try_into()?;
        let values: Vec<Ipld> = list.pop().unwrap().try_into()?;
        let keys: Vec<Ipld> = list.pop().unwrap().try_into()?;
        let layer: usize = list.pop().unwrap().try_into()?;

        let links = {
            let result: Result<VecDeque<_>, _> =
                links.into_iter().map(|ipld| ipld.try_into()).collect();

            result?
        };

        let indexes = {
            let result: Result<VecDeque<_>, _> =
                indexes.into_iter().map(|ipld| ipld.try_into()).collect();

            result?
        };

        let values = {
            let mut elements = VecDeque::with_capacity(values.len());
            for ipld in values {
                let Ok(value) = ipld.try_into() else {
                    return Err(Error::UnknownValueType);
                };

                elements.push_back(value);
            }

            elements
        };

        let keys = {
            let mut new_keys = VecDeque::with_capacity(keys.len());
            for ipld in keys {
                let Ok(key) = ipld.try_into() else {
                    return Err(Error::UnknownKeyType);
                };

                new_keys.push_back(key)
            }

            new_keys
        };

        Ok(Self {
            layer,
            keys,
            values,
            indexes,
            links,
        })
    }
}

impl<K: Key, V: Value> From<TreeNode<K, V>> for Ipld {
    fn from(node: TreeNode<K, V>) -> Self {
        let TreeNode {
            layer,
            keys,
            values,
            indexes,
            links,
        } = node;

        let keys = keys.into_iter().map(|key| key.into()).collect::<Vec<_>>();
        let keys = Ipld::List(keys);

        let values = values
            .into_iter()
            .map(|value| value.into())
            .collect::<Vec<_>>();
        let values = Ipld::List(values);

        let indexes = indexes
            .into_iter()
            .map(|idx| idx.into())
            .collect::<Vec<_>>();
        let indexes = Ipld::List(indexes);

        let links = links
            .into_iter()
            .map(|link| link.into())
            .collect::<Vec<_>>();
        let links = Ipld::List(links);

        Ipld::List(vec![layer.into(), keys, values, indexes, links])
    }
}

impl TryFrom<Ipld> for Config {
    type Error = Error;

    fn try_from(ipld: Ipld) -> Result<Self, Self::Error> {
        let mut list: Vec<Ipld> = ipld.try_into()?;

        if list.len() != 3 {
            return Err(DecodeError::RequireLength {
                name: "tuple",
                expect: 3,
                value: list.len(),
            }
            .into());
        };

        let multihash_code = {
            let hf: u64 = list.pop().unwrap().try_into()?;
            Code::try_from(hf)?
        };

        let codec = {
            let codec: i128 = list.pop().unwrap().try_into()?;
            let Some(codec) = Codec::from_i128(codec) else {
                return Err(Error::UnknownCodec);
            };
            codec
        };

        let base = list.pop().unwrap().try_into()?;

        let config = Self {
            base,
            codec,
            multihash_code,
        };

        Ok(config)
    }
}

impl From<Config> for Ipld {
    fn from(config: Config) -> Self {
        let Config {
            base,
            codec,
            multihash_code,
        } = config;

        let base = Ipld::Integer(base as i128);
        let codec = Ipld::Integer(codec as i128);
        let multihash_code = Ipld::Integer(u64::from(multihash_code) as i128);

        Ipld::List(vec![base, codec, multihash_code])
    }
}

impl From<Tree> for Ipld {
    fn from(tree: Tree) -> Self {
        let Tree { config, root } = tree;

        Ipld::List(vec![Ipld::Link(config), Ipld::Link(root)])
    }
}

impl TryFrom<Ipld> for Tree {
    type Error = Error;

    fn try_from(ipld: Ipld) -> Result<Self, Self::Error> {
        let mut list: Vec<Ipld> = ipld.try_into()?;

        if list.len() != 2 {
            return Err(DecodeError::RequireLength {
                name: "tuple",
                expect: 2,
                value: list.len(),
            }
            .into());
        };

        let root = list.pop().unwrap().try_into()?;
        let config = list.pop().unwrap().try_into()?;

        let tree = Self { config, root };

        Ok(tree)
    }
}

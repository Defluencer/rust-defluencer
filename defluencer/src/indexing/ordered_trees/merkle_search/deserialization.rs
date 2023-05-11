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

// Can link indices be omited for a more compact representation?

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
        let indices: Vec<Ipld> = list.pop().unwrap().try_into()?;
        let values: Vec<Ipld> = list.pop().unwrap().try_into()?;
        let keys: Vec<Ipld> = list.pop().unwrap().try_into()?;
        let layer: usize = list.pop().unwrap().try_into()?;

        let links = {
            let result: Result<VecDeque<_>, _> =
                links.into_iter().map(|ipld| ipld.try_into()).collect();

            result?
        };

        let indices = {
            let result: Result<VecDeque<_>, _> =
                indices.into_iter().map(|ipld| ipld.try_into()).collect();

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
            indices,
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
            indices,
            links,
        } = node;

        let keys = keys.into_iter().map(|key| key.into()).collect::<Vec<_>>();
        let keys = Ipld::List(keys);

        let values = values
            .into_iter()
            .map(|value| value.into())
            .collect::<Vec<_>>();
        let values = Ipld::List(values);

        let indices = indices
            .into_iter()
            .map(|idx| idx.into())
            .collect::<Vec<_>>();
        let indices = Ipld::List(indices);

        let links = links
            .into_iter()
            .map(|link| link.into())
            .collect::<Vec<_>>();
        let links = Ipld::List(links);

        Ipld::List(vec![layer.into(), keys, values, indices, links])
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

#[cfg(test)]
mod tests {
    use cid::Cid;

    use super::*;

    #[test]
    fn serde_roundtrip() {
        let key_one = 255u16;
        let key_two = 256u16;

        let value_one = String::from("This is value number one");
        let value_two = String::from("This is value number two");

        let link_one =
            Cid::try_from("bafkreifdpvsjgvfqtm6ko6hzppibabrrke3peky3pfgjdpje25ub64atqa").unwrap();
        let link_two =
            Cid::try_from("bafkreic3bbguse6e5zziexunbvagwlt6zkmrjhy5nehowroelzhisff5ua").unwrap();

        let node = TreeNode {
            layer: 1,
            keys: VecDeque::from([key_one.clone(), key_two.clone()]),
            values: VecDeque::from(vec![value_one.clone(), value_two.clone()]),
            indices: VecDeque::from(vec![0, 3]),
            links: VecDeque::from(vec![link_one.clone(), link_two.clone()]),
        };

        let encoded = serde_ipld_dagcbor::to_vec(&node).unwrap();
        let decoded: TreeNode<u16, String> = serde_ipld_dagcbor::from_slice(&encoded).unwrap();

        assert_eq!(node, decoded);
    }
}

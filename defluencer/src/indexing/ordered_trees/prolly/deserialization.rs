use std::{
    collections::{BTreeMap, VecDeque},
    fmt::Debug,
    str::FromStr,
};

use ipfs_api::responses::Codec;

use multihash::Code;

use serde::{Deserialize, Serialize};

use serde_ipld_dagcbor::DecodeError;

use super::{
    config::{Config, HashThreshold, Strategies, Tree},
    node::{Branch, Leaf, TreeNode},
};

use libipld_core::ipld::Ipld;

use num::FromPrimitive;

use crate::indexing::ordered_trees::{
    errors::Error,
    traits::{Key, Value},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(bound = "K: Key, V: Value", try_from = "Ipld", into = "Ipld")]
pub enum TreeNodes<K, V> {
    Branch(TreeNode<K, Branch>),
    Leaf(TreeNode<K, Leaf<V>>),
}

impl<K: Key, V: Value> From<TreeNodes<K, V>> for Ipld {
    fn from(node: TreeNodes<K, V>) -> Self {
        match node {
            TreeNodes::Branch(branch) => {
                let keys = branch
                    .keys
                    .into_iter()
                    .map(|key| key.into())
                    .collect::<Vec<_>>();
                let keys = Ipld::List(keys);

                let links = branch
                    .values
                    .links
                    .into_iter()
                    .map(|cid| cid.into())
                    .collect::<Vec<_>>();
                let links = Ipld::List(links);

                Ipld::List(vec![Ipld::Bool(false), keys, links])
            }
            TreeNodes::Leaf(leaf) => {
                let keys = leaf
                    .keys
                    .into_iter()
                    .map(|key| key.into())
                    .collect::<Vec<_>>();
                let keys = Ipld::List(keys);

                let values = leaf
                    .values
                    .elements
                    .into_iter()
                    .map(|value| value.into())
                    .collect::<Vec<_>>();
                let values = Ipld::List(values);

                Ipld::List(vec![Ipld::Bool(true), keys, values])
            }
        }
    }
}

impl<K: Key, V: Value> TryFrom<Ipld> for TreeNodes<K, V> {
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

        let values: Vec<Ipld> = list.pop().unwrap().try_into()?;
        let keys: Vec<Ipld> = list.pop().unwrap().try_into()?;
        let leaf: bool = list.pop().unwrap().try_into()?;

        let keys = {
            //TODO Find how to fix trait bounds
            /* let result: Result<Vec<K>, _> = keys.into_iter().map(|ipld| ipld.try_into()).collect();

            result? */

            let mut new_keys = VecDeque::with_capacity(keys.len());
            for ipld in keys {
                let Ok(key) = ipld.try_into() else {
                    return Err(Error::UnknownKeyType);
                };

                new_keys.push_back(key)
            }

            new_keys
        };

        let tree = if leaf {
            //TODO Find how to fix trait bounds
            /* let result: Result<Vec<V>, _> =
                values.into_iter().map(|ipld| ipld.try_into()).collect();

            let elements = result?; */

            let mut elements = Vec::with_capacity(values.len());
            for ipld in values {
                let Ok(value) = ipld.try_into() else {
                    return Err(Error::UnknownValueType);
                };

                elements.push(value);
            }

            let values = Leaf { elements };

            TreeNodes::Leaf(TreeNode { keys, values })
        } else {
            let result: Result<VecDeque<_>, _> =
                values.into_iter().map(|ipld| ipld.try_into()).collect();

            let links = result?;

            let values = Branch { links };

            TreeNodes::Branch(TreeNode { keys, values })
        };

        Ok(tree)
    }
}

impl TryFrom<Ipld> for Config {
    type Error = Error;

    fn try_from(ipld: Ipld) -> Result<Self, Self::Error> {
        let mut list: Vec<Ipld> = ipld.try_into()?;

        if list.len() != 7 {
            return Err(DecodeError::RequireLength {
                name: "tuple",
                expect: 7,
                value: list.len(),
            }
            .into());
        };

        let chunking_strategy = {
            let mut map: BTreeMap<String, Ipld> = list.pop().unwrap().try_into()?;

            let Some((key, value)) = map.pop_last() else {
                return Err(DecodeError::RequireLength { name: "map", expect: 1, value: map.len() }.into());
            };

            let Ok(mut chunking_strategy) = Strategies::from_str(&key) else {
                return Err(Error::UnknownChunkingStrategy);
            };

            match chunking_strategy {
                Strategies::Threshold(ref mut threshold) => {
                    let mut list: Vec<Ipld> = value.try_into()?;

                    if list.len() != 2 {
                        return Err(DecodeError::RequireLength {
                            name: "tuple",
                            expect: 2,
                            value: list.len(),
                        }
                        .into());
                    };

                    let hf: u64 = list.pop().unwrap().try_into()?;
                    let multihash_code = Code::try_from(hf)?;

                    let chunking_factor = list.pop().unwrap().try_into()?;

                    *threshold = HashThreshold {
                        chunking_factor,
                        multihash_code,
                    };
                }
            }

            chunking_strategy
        };

        let hash_length = list.pop().unwrap().try_into()?;

        let hash_function = {
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

        let cid_version = list.pop().unwrap().try_into()?;
        let max_size = list.pop().unwrap().try_into()?;
        let min_size = list.pop().unwrap().try_into()?;

        let config = Self {
            min_size,
            max_size,
            cid_version,
            codec,
            multihash_code: hash_function,
            hash_length,
            chunking_strategy,
        };

        Ok(config)
    }
}

impl From<Config> for Ipld {
    fn from(config: Config) -> Self {
        let Config {
            min_size,
            max_size,
            cid_version,
            codec,
            multihash_code: hash_function,
            hash_length,
            chunking_strategy,
        } = config;

        let min_size = Ipld::Integer(min_size as i128);
        let max_size = Ipld::Integer(max_size as i128);
        let cid_version = Ipld::Integer(cid_version as i128);
        let codec = Ipld::Integer(codec as i128);
        let hash_function = Ipld::Integer(u64::from(hash_function) as i128);
        let hash_length = match hash_length {
            Some(int) => Ipld::Integer(int as i128),
            None => Ipld::Null,
        };

        let chunking_strategy = match chunking_strategy {
            Strategies::Threshold(ref threshold) => {
                let HashThreshold {
                    chunking_factor,
                    multihash_code: hash_function,
                } = threshold;

                let map = BTreeMap::from([(
                    chunking_strategy.to_string(),
                    Ipld::List(vec![
                        Ipld::Integer(*chunking_factor as i128),
                        Ipld::Integer(u64::from(*hash_function) as i128),
                    ]),
                )]);

                Ipld::Map(map)
            }
        };

        Ipld::List(vec![
            min_size,
            max_size,
            cid_version,
            codec,
            hash_function,
            hash_length,
            chunking_strategy,
        ])
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
        let key_one = vec![255u8, 0u8];
        let key_two = vec![255u8, 1u8];

        let value_one = String::from("This is value number one");
        let value_two = String::from("This is value number two");

        let link_one =
            Cid::try_from("bafkreifdpvsjgvfqtm6ko6hzppibabrrke3peky3pfgjdpje25ub64atqa").unwrap();
        let link_two =
            Cid::try_from("bafkreic3bbguse6e5zziexunbvagwlt6zkmrjhy5nehowroelzhisff5ua").unwrap();

        let leaf_node = TreeNode {
            keys: VecDeque::from([key_one.clone(), key_two.clone()]),
            values: Leaf {
                elements: vec![value_one.clone(), value_two.clone()],
            },
        };
        let leaf_node = TreeNodes::Leaf(leaf_node);
        let encoded_leaf = serde_ipld_dagcbor::to_vec(&leaf_node).unwrap();
        let decoded_leaf: TreeNodes<Vec<u8>, String> =
            serde_ipld_dagcbor::from_slice(&encoded_leaf).unwrap();

        assert_eq!(leaf_node, decoded_leaf);

        let branch_node = TreeNode {
            keys: VecDeque::from([key_one, key_two]),
            values: Branch {
                links: VecDeque::from([link_one, link_two]),
            },
        };
        let branch_node = TreeNodes::Branch(branch_node);
        let encoded_branch = serde_ipld_dagcbor::to_vec(&branch_node).unwrap();
        let decoded_branch: TreeNodes<Vec<u8>, String> =
            serde_ipld_dagcbor::from_slice(&encoded_branch).unwrap();

        assert_eq!(branch_node, decoded_branch);
    }
}

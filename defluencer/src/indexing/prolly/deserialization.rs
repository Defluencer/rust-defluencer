use std::{collections::BTreeMap, fmt::Debug, str::FromStr};

use crate::errors::Error;

use ipfs_api::responses::Codec;
use multihash::Code;
use serde::{Deserialize, Serialize};

use super::{
    config::{Config, HashThreshold, Strategies, Tree},
    tree::{Branch, Key, Leaf, TreeNode, Value},
};

use libipld_core::ipld::Ipld;

use num::FromPrimitive;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(bound = "K: Key, V: Value", try_from = "Ipld", into = "Ipld")]
pub enum TreeNodes<K, V> {
    Branch(TreeNode<K, Branch>),
    Leaf(TreeNode<K, Leaf<V>>),
}

// Is there a way to not use Ipld enum as intermediate representation???

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

//TODO add meaningful errors
//TODO check if nodes have more data then the spec???

impl<K: Key, V: Value> TryFrom<Ipld> for TreeNodes<K, V> {
    type Error = Error;

    fn try_from(ipld: Ipld) -> Result<Self, Self::Error> {
        let Ipld::List(mut list) =  ipld else {
            return Err(Error::NotFound);
        };

        let Ipld::List(values) =  list.remove(2) else {
            return Err(Error::NotFound);
        };

        let Ipld::List(keys) =  list.remove(1) else {
            return Err(Error::NotFound);
        };

        let Ipld::Bool(is_leaf) =  list.remove(0) else {
             return Err(Error::NotFound);
        };

        let mut new_keys = Vec::with_capacity(keys.len());
        for ipld in keys {
            let Ok(key) = ipld.try_into() else {
                return Err(Error::NotFound);
            };

            new_keys.push(key);
        }
        let keys = new_keys;

        let tree = if is_leaf {
            let mut elements = Vec::with_capacity(values.len());
            for ipld in values {
                let Ok(value) = ipld.try_into() else {
                    return Err(Error::NotFound);
                };

                elements.push(value);
            }

            let values = Leaf { elements };

            TreeNodes::Leaf(TreeNode { keys, values })
        } else {
            let mut links = Vec::with_capacity(values.len());
            for ipld in values {
                let Ipld::Link(cid) = ipld else {
                    return Err(Error::NotFound);
                };

                links.push(cid)
            }

            let values = Branch { links };

            TreeNodes::Branch(TreeNode { keys, values })
        };

        Ok(tree)
    }
}

impl TryFrom<Ipld> for Config {
    type Error = Error;

    fn try_from(ipld: Ipld) -> Result<Self, Self::Error> {
        let Ipld::List(mut list) =  ipld else {
            return Err(Error::NotFound);
        };

        let chunking_strategy = {
            let Some(Ipld::Map(mut map)) =  list.pop() else {
                return Err(Error::NotFound);
            };

            let Some((key, value)) = map.pop_last() else {
                return Err(Error::NotFound);
            };

            let Ok(mut chunking_strategy) = Strategies::from_str(&key) else {
                return Err(Error::NotFound);
            };

            match chunking_strategy {
                Strategies::Threshold(ref mut threshold) => {
                    let Ipld::List(mut list) =  value else {
                        return Err(Error::NotFound);
                    };

                    let Some(Ipld::Integer(hash_function)) =  list.pop() else {
                        return Err(Error::NotFound);
                    };
                    let Ok(hash_function) = Code::try_from(hash_function as u64) else{
                        return Err(Error::NotFound);
                    };

                    let Some(Ipld::Integer(chunking_factor)) =  list.pop() else {
                        return Err(Error::NotFound);
                    };
                    let chunking_factor = chunking_factor as usize;

                    *threshold = HashThreshold {
                        chunking_factor,
                        hash_function,
                    };
                }
            }

            chunking_strategy
        };

        let hash_length = {
            let Some(Ipld::Integer(hash_length)) =  list.pop() else {
                return Err(Error::NotFound);
            };

            hash_length as usize
        };

        let hash_function = {
            let Some(Ipld::Integer(hash_function)) =  list.pop() else {
                return Err(Error::NotFound);
            };

            let Ok(hash_function) = Code::try_from(hash_function as u64) else{
                return Err(Error::NotFound);
            };

            hash_function
        };

        let codec = {
            let Some(Ipld::Integer(codec)) =  list.pop() else {
                return Err(Error::NotFound);
            };

            let Some(codec) = Codec::from_i128(codec) else {
                return Err(Error::NotFound);
            };

            codec
        };

        let cid_version = {
            let Some(Ipld::Integer(cid_version)) =  list.pop() else {
                return Err(Error::NotFound);
            };

            cid_version as usize
        };

        let max_size = {
            let Some(Ipld::Integer(max_size)) =  list.pop() else {
                return Err(Error::NotFound);
            };

            max_size as usize
        };

        let min_size = {
            let Some(Ipld::Integer(min_size)) =  list.pop() else {
                return Err(Error::NotFound);
            };

            min_size as usize
        };

        let config = Self {
            min_size,
            max_size,
            cid_version,
            codec,
            hash_function,
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
            hash_function,
            hash_length,
            chunking_strategy,
        } = config;

        let min_size = Ipld::Integer(min_size as i128);
        let max_size = Ipld::Integer(max_size as i128);
        let cid_version = Ipld::Integer(cid_version as i128);
        let codec = Ipld::Integer(codec as i128);
        let hash_function = Ipld::Integer(u64::from(hash_function) as i128);
        let hash_length = Ipld::Integer(hash_length as i128);

        let chunking_strategy = match chunking_strategy {
            Strategies::Threshold(threshold) => {
                let HashThreshold {
                    chunking_factor,
                    hash_function,
                } = threshold;

                let map = BTreeMap::from([(
                    chunking_strategy.to_string(),
                    Ipld::List(vec![
                        Ipld::Integer(chunking_factor as i128),
                        Ipld::Integer(u64::from(hash_function) as i128),
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
        let Ipld::List(mut list) =  ipld else {
            return Err(Error::NotFound);
        };

        let Some(Ipld::Link(root)) =  list.pop() else {
                return Err(Error::NotFound);
            };

        let Some(Ipld::Link(config)) =  list.pop() else {
                return Err(Error::NotFound);
            };

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
            keys: vec![key_one.clone(), key_two.clone()],
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
            keys: vec![key_one, key_two],
            values: Branch {
                links: vec![link_one, link_two],
            },
        };
        let branch_node = TreeNodes::Branch(branch_node);
        let encoded_branch = serde_ipld_dagcbor::to_vec(&branch_node).unwrap();
        let decoded_branch: TreeNodes<Vec<u8>, String> =
            serde_ipld_dagcbor::from_slice(&encoded_branch).unwrap();

        assert_eq!(branch_node, decoded_branch);
    }
}

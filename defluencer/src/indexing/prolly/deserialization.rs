use std::fmt::Debug;

use crate::errors::Error;

use serde::{Deserialize, Serialize};

use super::tree::{Branch, Key, Leaf, TreeNode, Value};

use libipld_core::ipld::Ipld;

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

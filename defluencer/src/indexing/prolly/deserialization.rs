use std::fmt::Debug;

use crate::errors::Error;

use serde::{Deserialize, Serialize};

use super::tree::{Branch, Key, Leaf, TreeNode, Value};

use libipld_core::ipld::Ipld;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "Ipld", into = "Ipld")]
pub enum TreeNodes<K, V> {
    #[serde(bound = "K: Key")]
    Branch(TreeNode<K, Branch>),
    #[serde(bound = "V: Value")]
    Leaf(TreeNode<K, Leaf<V>>),
}

//TODO add meaningful errors

/* impl<K: Key, V: Value> Serialize for TreeNodes<K, V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            TreeNodes::Branch(branch_node) => {
                let length = 1 + branch_node.keys.len() + branch_node.values.links.len();
                let mut seq = serializer.serialize_seq(Some(length))?;

                seq.serialize_element(&false)?;

                for key in branch_node.keys.iter() {
                    seq.serialize_element(key)?;
                }

                for link in branch_node.values.links.iter() {
                    seq.serialize_element(&link)?;
                }

                seq.end()
            }
            TreeNodes::Leaf(leaf_node) => {
                let length = 1 + leaf_node.keys.len() + leaf_node.values.elements.len();
                let mut seq = serializer.serialize_seq(Some(length))?;

                seq.serialize_element(&true)?;

                for key in leaf_node.keys.iter() {
                    seq.serialize_element(key)?;
                }

                for element in leaf_node.values.elements.iter() {
                    seq.serialize_element(element)?;
                }

                seq.end()
            }
        }
    }
} */

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
    use super::*;

    /* #[test]
    fn serde_roundtrip() {
        let leaf_node = TreeNode {
            keys: vec![
                vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0],
                vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 1],
            ],
            values: Leaf {
                elements: vec![
                    vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0],
                    vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 1],
                ],
            },
        };

        let treenum = TreeNodes::Leaf(leaf_node);

        let encoded = serde_ipld_dagcbor::to_vec(&treenum).unwrap();
        println!("Encoded: {:?}", encoded);

        let leaf_node_eq = Ipld::List(vec![
            Ipld::Bool(true),
            Ipld::Bytes(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0]),
            Ipld::Bytes(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 1]),
            Ipld::Bytes(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0]),
            Ipld::Bytes(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 1]),
        ]);

        let encoded_eq = serde_ipld_dagcbor::to_vec(&leaf_node_eq).unwrap();
        println!("Encoded EQ: {:?}", encoded_eq);

        assert_eq!(encoded, encoded_eq);

        let decoded: TreeNodes<Vec<u8>, Vec<u8>> =
            serde_ipld_dagcbor::from_slice(&encoded).unwrap();

        println!("Decoded: {:?}", decoded);

        assert_eq!(treenum, decoded)
    } */
}

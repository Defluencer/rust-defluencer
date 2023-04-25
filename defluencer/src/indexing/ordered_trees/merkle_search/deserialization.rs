use super::{
    config::{Config, Tree},
    tree::TreeNode,
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

impl<K: Key, V: Value> TryFrom<Ipld> for TreeNode<K, V> {
    type Error = Error;

    fn try_from(value: Ipld) -> Result<Self, Self::Error> {
        todo!()
    }
}

impl<K: Key, V: Value> From<TreeNode<K, V>> for Ipld {
    fn from(node: TreeNode<K, V>) -> Self {
        todo!()
    }
}

impl TryFrom<Ipld> for Config {
    type Error = Error;

    fn try_from(ipld: Ipld) -> Result<Self, Self::Error> {
        let mut list: Vec<Ipld> = ipld.try_into()?;

        if list.len() != 3 {
            return Err(DecodeError::RequireLength {
                name: "tuple",
                expect: 7,
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

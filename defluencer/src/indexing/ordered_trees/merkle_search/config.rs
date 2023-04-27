use cid::Cid;

use libipld_core::ipld::Ipld;

use multihash::{Code, MultihashDigest};

use ipfs_api::responses::Codec;

use num::{BigUint, Integer, Zero};

use serde::{Deserialize, Serialize};

use crate::indexing::ordered_trees::{errors::Error, traits::Key};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "Ipld", into = "Ipld")]
pub struct Config {
    /// Number Base (Radix)
    pub base: usize,

    /// IPLD codec.
    pub codec: Codec,

    /// Content identifiers hash function.
    pub multihash_code: Code,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            base: 16,
            codec: Codec::DagCbor,
            multihash_code: Code::Sha2_512,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "Ipld", into = "Ipld")]
pub struct Tree {
    pub config: Cid,
    pub root: Cid,
}

/// Using Horner's method but shortcircuit when first trailling non-zero is reached in the new base.
///
/// https://blogs.sas.com/content/iml/2022/09/12/convert-base-10.html
pub fn calculate_layer(config: &Config, key: impl Key) -> Result<usize, Error> {
    let base = BigUint::from(config.base);

    let bytes = match config.codec {
        Codec::DagCbor => serde_ipld_dagcbor::to_vec(&key.into())?,
        Codec::DagJson => serde_json::to_vec(&key.into())?,
        _ => unimplemented!(),
    };

    let multihash = config.multihash_code.digest(&bytes);

    // Big endian because you treat the bits as a number reading it from left to right.
    let hash_as_numb = BigUint::from_bytes_be(multihash.digest());

    let mut quotient = hash_as_numb;
    let mut remainder;

    let mut zero_count = 0;

    loop {
        (quotient, remainder) = quotient.div_rem(&base);

        if remainder != BigUint::zero() {
            break;
        }

        zero_count += 1;
    }

    Ok(zero_count)
}

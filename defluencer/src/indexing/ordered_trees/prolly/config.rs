use cid::Cid;

use multihash::{Code, MultihashDigest};

use serde::{Deserialize, Serialize};

use ipfs_api::responses::Codec;

use libipld_core::ipld::Ipld;

use strum::{Display, EnumString};

use crate::indexing::ordered_trees::{
    errors::Error,
    traits::{Key, Value},
};

/// Chunking is the strategy of determining chunk boundaries:
/// Given a list of key-value pairs, it 'decides' which are still inside node A and
/// which already go to the next node B on the same level.
#[derive(Display, Debug, Clone, EnumString, PartialEq, Eq)]
pub enum Strategies {
    #[strum(serialize = "hashThreshold")]
    Threshold(HashThreshold),
}

impl Default for Strategies {
    fn default() -> Self {
        Self::Threshold(HashThreshold::default())
    }
}

/// Chunking strategy that count 0 bits in the last 4 bytes of a key's hash then
/// compare it with the chunking factor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashThreshold {
    pub chunking_factor: usize,
    pub multihash_code: Code,
}

impl Default for HashThreshold {
    fn default() -> Self {
        Self {
            chunking_factor: 1 << 26,
            multihash_code: Code::Sha2_256,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "Ipld", into = "Ipld")]
pub struct Config {
    /// Minimum chunk size in bytes.
    pub min_size: usize,

    /// Maximum chunk size in bytes.
    pub max_size: usize,

    /// Content identifier version.
    pub cid_version: usize,

    /// IPLD codec.
    pub codec: Codec,

    /// Content identifiers hash function.
    pub multihash_code: Code,

    /// Content identifiers hash length in bytes.
    pub hash_length: Option<usize>,

    /// Strategy used to shape the tree.
    pub chunking_strategy: Strategies,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            min_size: 0,
            max_size: 1048572,
            cid_version: 1,
            codec: Codec::DagCbor,
            multihash_code: Code::Sha2_256,
            hash_length: None,
            chunking_strategy: Strategies::default(),
        }
    }
}

impl Config {
    pub fn boundary(&mut self, key: impl Key, value: impl Value) -> Result<bool, Error> {
        match &self.chunking_strategy {
            Strategies::Threshold(threshold) => {
                let mut bytes = match self.codec {
                    Codec::DagCbor => serde_ipld_dagcbor::to_vec(&key.into())?,
                    Codec::DagJson => serde_json::to_vec(&key.into())?,
                    _ => unimplemented!("IPLD Codec"),
                };

                let mut value_bytes = match self.codec {
                    Codec::DagCbor => serde_ipld_dagcbor::to_vec(&value.into())?,
                    Codec::DagJson => serde_json::to_vec(&value.into())?,
                    _ => unimplemented!("IPLD Codec"),
                };

                bytes.append(&mut value_bytes);

                let hash = threshold.multihash_code.digest(&bytes);

                let bound = chunking(threshold.chunking_factor as u32, hash.digest());

                Ok(bound)
            }
        }
    }
}

fn chunking(factor: u32, hash: &[u8]) -> bool {
    let zero_count: u32 = hash
        .into_iter()
        .rev()
        .take(4)
        .map(|byte| byte.count_zeros())
        .sum();

    let threshold = (u32::MAX / factor).count_zeros();

    zero_count > threshold
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "Ipld", into = "Ipld")]
pub struct Tree {
    pub config: Cid,
    pub root: Cid,
}

#[cfg(test)]
mod tests {
    use super::*;

    use rand_xoshiro::{
        rand_core::{RngCore, SeedableRng},
        Xoshiro256StarStar,
    };

    #[test]
    fn bounds() {
        let mut rng = Xoshiro256StarStar::from_entropy();
        let mut hash = [0u8; 4];
        let factor = 1 << 22;
        let threshold = (u32::MAX / factor).count_zeros();

        for _ in 0..10000 {
            rng.fill_bytes(&mut hash);

            let mut binary = format!("{:b}", u32::from_be_bytes(hash));

            let mut binary_string_zero_count = 32;
            while let Some(char) = binary.pop() {
                if char != '0' {
                    binary_string_zero_count -= 1;
                }
            }

            let expected_bound = binary_string_zero_count > threshold;

            let bound = chunking(factor, &hash);

            assert_eq!(bound, expected_bound);
        }
    }
}

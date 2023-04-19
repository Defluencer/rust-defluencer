use cid::Cid;

use multihash::{Code, MultihashDigest};

use serde::{Deserialize, Serialize};

use ipfs_api::responses::Codec;

use libipld_core::ipld::Ipld;
use strum::{Display, EnumString};

use super::tree::Key;

//TODO Find better abstraction for chunking strategy. Kinda hard to abstract when there's only one example!

pub trait ChunkingStrategy {
    fn boundary(&self, input: impl Key) -> bool;
}

#[derive(Display, Debug, Clone, Copy, EnumString)]
pub enum Strategies {
    #[strum(serialize = "hashThreshold")]
    Threshold(HashThreshold),
}

impl Default for Strategies {
    fn default() -> Self {
        Self::Threshold(HashThreshold::default())
    }
}

impl ChunkingStrategy for Strategies {
    fn boundary(&self, input: impl Key) -> bool {
        match self {
            Strategies::Threshold(strat) => strat.boundary(input),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HashThreshold {
    pub chunking_factor: usize,
    pub hash_function: Code,
}

impl Default for HashThreshold {
    fn default() -> Self {
        Self {
            chunking_factor: 16,
            hash_function: Code::Sha2_256,
        }
    }
}

impl ChunkingStrategy for HashThreshold {
    fn boundary(&self, input: impl Key) -> bool {
        let ipld: Ipld = input.into();

        let hash = match ipld {
            Ipld::Bool(bool) => self.hash_function.digest(&[(bool as u8)]),
            Ipld::Integer(int) => self.hash_function.digest(&int.to_ne_bytes()),
            Ipld::String(string) => self.hash_function.digest(string.as_bytes()),
            Ipld::Bytes(bytes) => self.hash_function.digest(&bytes),
            Ipld::Link(cid) => *cid.hash(),
            _ => panic!("Keys cannot be this Ipld variant"),
        };

        let zero_count: u32 = hash
            .digest()
            .into_iter()
            .rev()
            .take(4)
            .map(|byte| byte.count_zeros())
            .sum();

        let threshold = (u32::MAX / self.chunking_factor as u32).count_zeros();

        zero_count > threshold
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "Ipld", into = "Ipld")]
pub struct Config {
    pub min_size: usize,
    pub max_size: usize,
    pub cid_version: usize,
    pub codec: Codec,
    pub hash_function: Code,
    pub hash_length: usize,
    pub chunking_strategy: Strategies,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            min_size: 0,
            max_size: 1048576,
            cid_version: 1,
            codec: Codec::DagCbor,
            hash_function: Code::Sha2_256,
            hash_length: 0,
            chunking_strategy: Strategies::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "Ipld", into = "Ipld")]
pub struct Tree {
    pub config: Cid,
    pub root: Cid,
}

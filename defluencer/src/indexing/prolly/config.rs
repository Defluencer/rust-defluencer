use linked_data::types::IPLDLink;

use multihash::{Code, MultihashDigest};

use serde::{Deserialize, Serialize};

use ipfs_api::responses::Codec;

use libipld_core::ipld::Ipld;

use super::tree::Key;

pub trait ChunkingStrategy {
    fn boundary(&self, input: impl Key) -> bool;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Strategies {
    #[serde(rename = "hashThreshold")]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HashThreshold(u32, Code);

impl Default for HashThreshold {
    fn default() -> Self {
        Self(16, Code::Sha2_256)
    }
}

impl ChunkingStrategy for HashThreshold {
    fn boundary(&self, input: impl Key) -> bool {
        //TODO
        let ipld: Ipld = input.into();

        let hash = match ipld {
            Ipld::Bool(bool) => self.1.digest(&[(bool as u8)]),
            Ipld::Integer(int) => self.1.digest(&int.to_ne_bytes()),
            Ipld::String(string) => self.1.digest(string.as_bytes()),
            Ipld::Bytes(bytes) => self.1.digest(&bytes),
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

        let threshold = (u32::MAX / self.0).count_zeros();

        zero_count > threshold
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config(usize, usize, usize, Codec, Code, usize, Strategies);

impl Default for Config {
    fn default() -> Self {
        Self(
            0,
            1048576,
            1,
            Codec::DagCbor,
            Code::Sha2_256,
            0,
            Strategies::default(),
        )
    }
}

impl Config {
    pub fn min_size(&self) -> usize {
        self.0
    }

    pub fn max_size(&self) -> usize {
        self.1
    }

    pub fn cid_version(&self) -> usize {
        self.2
    }

    pub fn codec(&self) -> Codec {
        self.3
    }

    pub fn hash_fn(&self) -> Code {
        self.4
    }

    pub fn hash_len(&self) -> usize {
        self.5
    }

    pub fn chunking_strat(&self) -> Strategies {
        self.6
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Tree(IPLDLink, IPLDLink);

impl Tree {
    pub fn config(&self) -> IPLDLink {
        self.0
    }

    pub fn root(&self) -> IPLDLink {
        self.1
    }

    pub fn into_inner(self) -> (IPLDLink, IPLDLink) {
        (self.0, self.1)
    }
}

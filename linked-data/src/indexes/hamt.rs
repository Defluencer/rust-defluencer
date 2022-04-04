use std::collections::VecDeque;

use bitvec::BitArr;

use cid::Cid;
use serde::{Deserialize, Serialize};

use crate::IPLDLink;

pub const BIT_WIDTH: usize = 8;
pub const DIGEST_LENGTH_BITS: usize = 2usize.pow(BIT_WIDTH as u32);
pub const DIGEST_LENGTH_BYTES: usize = DIGEST_LENGTH_BITS / 8;
pub const BUCKET_SIZE: usize = 3;

pub type BitField = BitArr!(for DIGEST_LENGTH_BITS, in u8);

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct HAMTRoot {
    #[serde(rename = "hashAlg")]
    pub hash_algorithm: usize, // 12 in most cases

    #[serde(rename = "bucketSize")]
    pub bucket_size: usize,

    pub hamt: HAMTNode,
}

impl Default for HAMTRoot {
    fn default() -> Self {
        Self {
            hash_algorithm: 12,
            bucket_size: 3,
            hamt: HAMTNode {
                map: [0u8; DIGEST_LENGTH_BYTES],
                data: vec![],
            },
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct HAMTNode {
    pub map: [u8; DIGEST_LENGTH_BYTES],
    pub data: Vec<Element>,
}

impl Default for HAMTNode {
    fn default() -> Self {
        let bitfield = BitField::ZERO;

        Self {
            map: bitfield.into_inner(),
            data: vec![],
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub enum Element {
    Link(IPLDLink),
    Bucket(VecDeque<BucketEntry>),
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct BucketEntry {
    pub key: IPLDLink,
    pub value: IPLDLink,
}

impl PartialEq for BucketEntry {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for BucketEntry {}

impl PartialOrd for BucketEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.key.partial_cmp(&other.key)
    }
}

impl Ord for BucketEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.key.cmp(&other.key)
    }
}

impl From<Cid> for BucketEntry {
    fn from(cid: Cid) -> Self {
        Self {
            key: cid.into(),
            value: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_test() {
        let bitfield = BitField::ZERO;

        let node = HAMTNode {
            map: bitfield.into_inner(),
            data: vec![],
        };

        let json = serde_json::to_string_pretty(&node).unwrap();

        println!("{}", json);
    }
}

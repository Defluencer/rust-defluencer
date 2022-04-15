use std::collections::BTreeSet;

use bitvec::BitArr;

use serde::{Deserialize, Serialize};

use crate::types::IPLDLink;

// https://ipld.io/specs/advanced-data-layouts/hamt/spec/#implementation-defaults
pub const HASH_ALGORITHM: usize = 0x12; // SHA2-256 => 32 bytes digest
pub const DIGEST_LENGTH_BYTES: usize = 32;

pub const BIT_WIDTH: usize = 8;
pub const BUCKET_SIZE: usize = 3;

pub const MAP_LENGTH_BITS: usize = 2usize.pow(BIT_WIDTH as u32);

pub type BitField = BitArr!(for MAP_LENGTH_BITS, in u8);

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct HAMTRoot {
    #[serde(rename = "hashAlg")]
    pub hash_algorithm: usize,

    #[serde(rename = "bucketSize")]
    pub bucket_size: usize,

    pub hamt: HAMTNode,
}

impl Default for HAMTRoot {
    fn default() -> Self {
        Self {
            hash_algorithm: HASH_ALGORITHM,
            bucket_size: BUCKET_SIZE,
            hamt: HAMTNode {
                map: [0u8; MAP_LENGTH_BITS / 8],
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
#[serde(untagged)]
pub enum Element {
    Link(IPLDLink),
    Bucket(BTreeSet<BucketEntry>),
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
pub struct BucketEntry {
    pub key: [u8; DIGEST_LENGTH_BYTES],
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

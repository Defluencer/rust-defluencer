use serde::{Deserialize, Serialize};

use crate::IPLDLink;

use bitvec::prelude::*;

//TODO Implement a HAMT.

//Need HAMT for channel comments and for aggregating.

pub const BIT_WIDTH: usize = 8;
pub const DIGEST_LENGTH_BITS: usize = 2usize.pow(BIT_WIDTH as u32);
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
                map: [0u8; 32],
                data: vec![],
            },
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct HAMTNode {
    pub map: [u8; 32],
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
    Bucket(Vec<BucketEntree>),
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct BucketEntree {
    pub key: IPLDLink,
    pub value: IPLDLink,
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

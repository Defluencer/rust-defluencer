use serde::{Deserialize, Serialize};

use crate::IPLDLink;

//TODO Implement a HAMT.

//Need HAMT for channel comments and for aggregating.

pub const BIT_WIDTH: u8 = 8;
pub const BUCKET_SIZE: u8 = 3;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct HAMTNode {
    pub map: u8,
    pub data: Vec<Element>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub enum Element {
    Link(IPLDLink),
    Bucket(Vec<BucketEntree>),
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct BucketEntree {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

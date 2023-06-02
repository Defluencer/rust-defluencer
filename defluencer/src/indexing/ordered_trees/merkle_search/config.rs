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
            multihash_code: Code::Sha2_256,
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
pub fn calculate_layer(config: &Config, key: impl Key) -> Result<usize, Error> {
    // https://blogs.sas.com/content/iml/2022/09/12/convert-base-10.html

    let bytes = match config.codec {
        Codec::DagCbor => serde_ipld_dagcbor::to_vec(&key.into())?,
        Codec::DagJson => serde_json::to_vec(&key.into())?,
        _ => unimplemented!(),
    };

    let multihash = config.multihash_code.digest(&bytes);

    let zero_count = horner(config.base, multihash.digest());

    Ok(zero_count)
}

fn horner(base: usize, hash: &[u8]) -> usize {
    let base = BigUint::from(base);

    // Big endian because you treat the bits as a number reading it from left to right.
    let hash_as_numb = BigUint::from_bytes_be(hash);

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

    zero_count
}

#[cfg(test)]
mod tests {
    use super::*;

    use rand_xoshiro::{
        rand_core::{RngCore, SeedableRng},
        Xoshiro256StarStar,
    };

    #[test]
    fn layer_calulation() {
        let mut rng = Xoshiro256StarStar::from_entropy();

        for _ in 0..100 {
            let hash = rng.next_u64();

            let mut hex = format!("{:#X}", hash);
            let mut hex_string_zero_count = 0;
            while let Some(last_char) = hex.pop() {
                if last_char != '0' {
                    break;
                }

                hex_string_zero_count += 1;
            }

            let zero_count = horner(16, &hash.to_be_bytes());

            assert_eq!(hex_string_zero_count, zero_count);

            let mut octal = format!("{:#o}", hash);
            let mut octal_string_zero_count = 0;
            while let Some(last_char) = octal.pop() {
                if last_char != '0' {
                    break;
                }

                octal_string_zero_count += 1;
            }

            let zero_count = horner(8, &hash.to_be_bytes());

            assert_eq!(octal_string_zero_count, zero_count);

            let mut binary = format!("{:#b}", hash);
            let mut binary_string_zero_count = 0;
            while let Some(last_char) = binary.pop() {
                if last_char != '0' {
                    break;
                }

                binary_string_zero_count += 1;
            }

            let zero_count = horner(2, &hash.to_be_bytes());

            assert_eq!(binary_string_zero_count, zero_count);
        }
    }
}

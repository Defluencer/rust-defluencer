mod errors;
mod tests;

pub use errors::Error;

use multihash::Multihash;

use sha2::{Digest, Sha256};

use signatory::{ecdsa::Secp256k1Signer, ed25519::Ed25519Signer};

use std::ops::Add;

use chrono::{Duration, SecondsFormat, Utc};

use cid::Cid;

use prost::{self, Enumeration, Message};

use strum::Display;

// https://github.com/libp2p/specs/blob/master/peer-ids/peer-ids.md#key-types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Enumeration)]
#[repr(i32)]
enum KeyType {
    RSA = 0,
    Ed25519 = 1,
    Secp256k1 = 2,
    ECDSA = 3,
}

// https://github.com/libp2p/specs/blob/master/peer-ids/peer-ids.md#keys
#[derive(Clone, PartialEq, Message)]
struct CryptoKey {
    #[prost(enumeration = "KeyType")]
    pub key_type: i32,

    #[prost(bytes)]
    pub data: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Enumeration, Display)]
#[repr(i32)]
pub enum ValidityType {
    EOL = 0,
}

// https://github.com/ipfs/specs/blob/master/IPNS.md#ipns-record
#[derive(Clone, PartialEq, Message)]
pub struct IPNSRecord {
    #[prost(bytes)]
    value: Vec<u8>,

    #[prost(bytes)]
    signature: Vec<u8>,

    #[prost(enumeration = "ValidityType")]
    validity_type: i32,

    #[prost(bytes)]
    validity: Vec<u8>,

    #[prost(uint64)]
    sequence: u64,

    #[prost(uint64)]
    ttl: u64,

    #[prost(bytes)]
    public_key: Vec<u8>,
}

impl IPNSRecord {
    pub fn from_bytes(data: &[u8]) -> Result<Self, Error> {
        let result = IPNSRecord::decode(data)?;
        Ok(result)
    }

    /// Return the Cid this record point to.
    pub fn get_value(&self) -> Cid {
        let cid_str = std::str::from_utf8(&self.value).expect("Stringified Cid");
        Cid::try_from(cid_str).expect("Cid Validity")
    }

    /// Return the number of updates this record had.
    pub fn get_sequence(&self) -> u64 {
        self.sequence
    }

    pub fn get_address(&self) -> Cid {
        let multihash = if self.public_key.len() <= 42 {
            Multihash::wrap(/* Identity */ 0x00, &self.public_key).expect("Valid Multihash")
        } else {
            let hash = Sha256::new_with_prefix(&self.public_key).finalize();

            Multihash::wrap(/* Sha256 */ 0x12, &hash).expect("Valid Multihash")
        };

        Cid::new_v1(/* Libp2p key */ 0x72, multihash)
    }

    /// Return an error if this record is not valid for the specified IPNS address.
    pub fn verify(&self, ipns_addr: Cid) -> Result<(), Error> {
        use signature::Verifier;

        if self.validity_type != 0 {
            panic!("Does ValidityType now has more than one variant?")
        }

        let validity_type = ValidityType::EOL;

        let signing_input = {
            let mut data = Vec::with_capacity(
                self.value.len() + self.validity.len() + 3, /* b"EOL".len() == 3 */
            );

            data.extend(self.value.iter());
            data.extend(self.validity.iter());

            data.extend(validity_type.to_string().as_bytes());

            data
        };

        let data = if !self.public_key.is_empty() {
            self.public_key.as_ref()
        } else {
            ipns_addr.hash().digest() // If the pub key is not in the record it fits in the addr
        };

        let CryptoKey { key_type, data } = CryptoKey::decode(data).expect("Crypto Key Decoding");

        match key_type {
            0/* RSA */ => unimplemented!(),
            1/* Ed25519 */ =>  {
                use ed25519_dalek::PublicKey;
                use ed25519::Signature;

                let public_key = PublicKey::from_bytes(&data).expect("Valid Key");
                let signature = Signature::from_bytes(&self.signature).expect("Valid Signature");

                public_key.verify(&signing_input, &signature)?;
            },
            2/* Secp256k1 */ => {
                use k256::ecdsa::VerifyingKey;
                use ecdsa::Signature as Sig;
                //use signature::Signature;

                let public_key = VerifyingKey::from_sec1_bytes(&data).expect("Valid Key");
                let signature = Sig::from_der(&self.signature).expect("Valid Signature");

                public_key.verify(&signing_input, &signature)?;
            },
            3/* KeyType::ECDSA */ => unimplemented!(),
            _ => panic!("Enum has only 4 possible values")
        }

        Ok(())
    }

    //TODO find a way to limit Signer to some hash algo used when signing so that it fits the IPNS parameters

    /// Create a new record using the EcDSA with the curve secp256k1.
    pub fn new_with_secp256k1(
        cid: Cid,
        valid_for: Duration,
        sequence: u64,
        signer: impl Secp256k1Signer,
    ) -> Result<Self, Error> {
        let value = format!("/ipfs/{}", cid.to_string()).into_bytes();

        let validity = Utc::now()
            .add(valid_for)
            .to_rfc3339_opts(SecondsFormat::Nanos, false)
            .into_bytes();

        let validity_type = ValidityType::EOL;

        let signing_input = {
            let mut data = Vec::with_capacity(
                value.len() + validity.len() + 3, /* b"EOL".len() == 3 */
            );

            data.extend(value.iter());
            data.extend(validity.iter());
            data.extend(validity_type.to_string().as_bytes());

            data
        };

        let signature = signer.try_sign(&signing_input)?;
        let verif_key = signer.verifying_key();

        let signature = signature.to_der().to_bytes().into_vec();

        let public_key = CryptoKey {
            key_type: KeyType::Secp256k1 as i32,
            data: verif_key.to_bytes().to_vec(),
        }
        .encode_to_vec(); // Protobuf encoding

        Ok(Self {
            value,
            signature,
            validity_type: validity_type as i32,
            validity,
            sequence,
            ttl: 0, //TODO figure this out!
            public_key,
        })
    }

    /// Create a new record using the EdDSA with the curve ed25519.
    pub fn new_with_ed25519(
        cid: Cid,
        valid_for: Duration,
        sequence: u64,
        signer: impl Ed25519Signer,
    ) -> Result<Self, Error> {
        let value = format!("/ipfs/{}", cid.to_string()).into_bytes();

        let validity = Utc::now()
            .add(valid_for)
            .to_rfc3339_opts(SecondsFormat::Nanos, false)
            .into_bytes();

        let validity_type = ValidityType::EOL;

        let signing_input = {
            let mut data = Vec::with_capacity(
                value.len() + validity.len() + 3, /* b"EOL".len() == 3 */
            );

            data.extend(value.iter());
            data.extend(validity.iter());
            data.extend(validity_type.to_string().as_bytes());

            data
        };

        let signature = signer.try_sign(&signing_input)?;
        let verif_key = signer.verifying_key();

        let signature = signature.to_bytes().to_vec();

        let public_key = CryptoKey {
            key_type: KeyType::Ed25519 as i32,
            data: verif_key.to_bytes().to_vec(),
        }
        .encode_to_vec(); // Protobuf encoding

        Ok(Self {
            value,
            signature,
            validity_type: validity_type as i32,
            validity,
            sequence,
            ttl: 0, //TODO figure this out!
            public_key,
        })
    }
}

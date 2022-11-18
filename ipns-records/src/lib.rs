mod errors;
mod tests;
mod traits;

use elliptic_curve::pkcs8::DecodePublicKey;

pub use errors::Error;

use serde::{Deserialize, Serialize};

use sha2::{Digest, Sha256};

use signature::Signature;

use traits::RecordSigner;

use std::ops::Add;

use chrono::{Duration, SecondsFormat, Utc};

use cid::Cid;

use multihash::MultihashGeneric;
type Multihash = MultihashGeneric<64>;

use prost::{self, Enumeration, Message};

use strum::Display;

/// Type of a record keys.
///
/// https://github.com/libp2p/specs/blob/master/peer-ids/peer-ids.md#key-types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Enumeration)]
#[repr(i32)]
pub enum KeyType {
    RSA = 0,
    Ed25519 = 1,
    Secp256k1 = 2,
    ECDSA = 3,
}

/// Protobuf encoded crypto keys.
///
/// https://github.com/libp2p/specs/blob/master/peer-ids/peer-ids.md#keys
#[derive(Clone, PartialEq, Message)]
pub struct CryptoKey {
    #[prost(enumeration = "KeyType")]
    pub r#type: i32,

    #[prost(bytes)]
    pub data: Vec<u8>,
}

/// Validity type only valid if EOL.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Enumeration,
    Display,
    Serialize,
    Deserialize,
)]
#[repr(i32)]
pub enum ValidityType {
    EOL = 0,
}

/// Protobuf encoded record.
///
/// https://github.com/ipfs/specs/blob/main/ipns/IPNS.md#record-serialization-format
#[derive(Clone, PartialEq, Message)]
pub struct IPNSRecord {
    #[prost(bytes)]
    value: Vec<u8>,

    #[prost(bytes)]
    signature_v1: Vec<u8>,

    #[prost(enumeration = "ValidityType")]
    validity_type: i32,

    #[prost(bytes)]
    validity: Vec<u8>,

    #[prost(uint64)]
    sequence: u64,

    #[prost(uint64)]
    ttl: u64,

    #[prost(bytes)]
    pub_key: Vec<u8>,

    #[prost(bytes)]
    signature_v2: Vec<u8>,

    #[prost(bytes)]
    data: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone)]
struct DagCborDocument {
    value: Vec<u8>,

    validity_type: ValidityType,

    validity: Vec<u8>,

    sequence: u64,

    ttl: u64,
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

    /// Return the IPNS address of this record.
    ///
    /// Public key less than 42 bytes are store as IPNS address digest
    pub fn get_address(&self) -> Option<Cid> {
        if self.pub_key.is_empty() {
            return None;
        }

        let hash = Sha256::new_with_prefix(&self.pub_key).finalize();
        let multihash = Multihash::wrap(/* Sha256 */ 0x12, &hash).expect("Valid Multihash");
        let cid = Cid::new_v1(/* Libp2p key */ 0x72, multihash);

        Some(cid)
    }

    /// Return an error if this record is not valid for the specified IPNS address.
    pub fn verify(&self, ipns_addr: Cid) -> Result<(), Error> {
        use signature::Verifier;

        if self.signature_v2.is_empty() {
            return Err(Error::EmptySignature);
        }

        if self.data.is_empty() {
            return Err(Error::EmptyData);
        }

        let data = if self.pub_key.is_empty() {
            ipns_addr.hash().digest()
        } else {
            let addr = {
                let hash = Sha256::new_with_prefix(&self.pub_key).finalize();
                let multihash = Multihash::wrap(/* Sha256 */ 0x12, &hash).expect("Valid Multihash");
                Cid::new_v1(/* Libp2p key */ 0x72, multihash)
            };

            if addr != ipns_addr {
                return Err(Error::AddressMismatch);
            }

            self.pub_key.as_ref()
        };

        let crypto_key = CryptoKey::decode(data)?;

        let document: DagCborDocument =
            serde_ipld_dagcbor::from_slice(&self.data).expect("Valid Dag Cbor");

        if document.value != self.value
            || document.validity != self.validity
            || document.validity_type != self.validity_type()
            || document.sequence != self.sequence
            || document.ttl != self.ttl
        {
            return Err(Error::DataMismatch);
        }

        //prefix
        let mut signing_input_v2: Vec<u8> = vec![
            0x69, 0x70, 0x6e, 0x73, 0x2d, 0x73, 0x69, 0x67, 0x6e, 0x61, 0x74, 0x75, 0x72, 0x65,
            0x3a,
        ];

        signing_input_v2.extend(self.data.iter());

        /* let signing_input_v1 = {
            let mut data = Vec::with_capacity(
                self.value.len() + self.validity.len() + 3, /* b"EOL".len() == 3 */
            );

            data.extend(self.value.iter());
            data.extend(self.validity.iter());

            data.extend(self.validity_type().to_string().as_bytes());

            data
        }; */

        match crypto_key.r#type() {
            KeyType::RSA => unimplemented!(),
            KeyType::Ed25519 => {
                use ed25519::Signature;
                use ed25519_dalek::PublicKey;

                let public_key = PublicKey::from_bytes(&crypto_key.data)?;
                let signature = Signature::from_bytes(&self.signature_v2)?;

                public_key.verify(&signing_input_v2, &signature)?;
            }
            KeyType::Secp256k1 => {
                use k256::ecdsa::Signature;
                use k256::ecdsa::VerifyingKey;

                let verif_key = VerifyingKey::from_sec1_bytes(&crypto_key.data)?;
                let signature = Signature::from_der(&self.signature_v2)?;

                verif_key.verify(&signing_input_v2, &signature)?;
            }
            KeyType::ECDSA => {
                use p256::ecdsa::Signature;
                use p256::ecdsa::VerifyingKey;

                let verif_key =
                    VerifyingKey::from_public_key_der(&crypto_key.data).expect("Valid Public Key");
                let signature = Signature::from_der(&self.signature_v2)?;

                verif_key.verify(&signing_input_v2, &signature)?;
            }
        }

        Ok(())
    }

    /// Create a new IPNS record.
    pub fn new<S, U>(
        cid: Cid,
        valid_for: Duration,
        sequence: u64,
        ttl: u64,
        signer: S,
    ) -> Result<Self, Error>
    where
        S: RecordSigner<U>,
        U: Signature,
    {
        let value = format!("/ipfs/{}", cid.to_string()).into_bytes();

        let validity = Utc::now()
            .add(valid_for)
            .to_rfc3339_opts(SecondsFormat::Nanos, false)
            .into_bytes();

        let validity_type = ValidityType::EOL;

        let signing_input_v1 = {
            let mut data = Vec::with_capacity(
                value.len() + validity.len() + 3, /* b"EOL".len() == 3 */
            );

            data.extend(value.iter());
            data.extend(validity.iter());
            data.extend(validity_type.to_string().as_bytes());

            data
        };

        let mut pub_key = signer.crypto_key().encode_to_vec(); // Protobuf encoding

        if pub_key.len() <= 42 {
            pub_key.clear();
        }

        let signature_v1 = signer.try_sign(&signing_input_v1)?;
        let signature_v1 = signature_v1.as_bytes().to_vec();

        let document = DagCborDocument {
            value: value.clone(),
            validity_type,
            validity: validity.clone(),
            sequence,
            ttl,
        };

        let data = serde_ipld_dagcbor::to_vec(&document).expect("Valid Dag Cbor");

        //prefix
        let mut signing_input_v2: Vec<u8> = vec![
            0x69, 0x70, 0x6e, 0x73, 0x2d, 0x73, 0x69, 0x67, 0x6e, 0x61, 0x74, 0x75, 0x72, 0x65,
            0x3a,
        ];

        signing_input_v2.extend(data.iter());

        let signature_v2 = signer.try_sign(&signing_input_v2)?;
        let signature_v2 = signature_v2.as_bytes().to_vec();

        Ok(Self {
            value,
            signature_v1,
            validity_type: validity_type as i32,
            validity,
            sequence,
            ttl,
            pub_key,
            signature_v2,
            data,
        })
    }
}

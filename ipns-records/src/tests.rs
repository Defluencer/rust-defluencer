#![cfg(test)]

use super::*;

use chrono::Duration;

use ecdsa::VerifyingKey;
use prost::Message;

use sha2::{Digest, Sha256};

use signature::{DigestSigner, Signer};

use crate::{CryptoKey, IPNSRecord, KeyType, RecordSigner};

pub struct Ed25519IPNSRecordSigner {
    pub signing_key: ed25519_dalek::SigningKey,
}

impl Signer<ed25519::Signature> for Ed25519IPNSRecordSigner {
    fn sign(&self, msg: &[u8]) -> ed25519::Signature {
        self.signing_key.sign(msg)
    }

    fn try_sign(&self, msg: &[u8]) -> Result<ed25519::Signature, signature::Error> {
        self.signing_key.try_sign(msg)
    }
}

impl RecordSigner<ed25519::Signature> for Ed25519IPNSRecordSigner {
    fn crypto_key(&self) -> CryptoKey {
        CryptoKey::new_ed15519_dalek(&self.signing_key.verifying_key())
    }
}

#[test]
fn ed25519_roundtrip() {
    let value =
        Cid::try_from("bafyreih223c6mqauz5ouolokqrofaekpuu45eblm33fm3g2rlwdkqfabo4").unwrap();

    let duration = Duration::days(30);

    let ttl = 0;
    let sequence = 0;

    
    use rand_core::OsRng;
    let mut csprng = OsRng {};
    let signing_key = ed25519_dalek::SigningKey::generate(&mut csprng);

    let addr = {
        let public_key = CryptoKey {
            r#type: KeyType::Ed25519 as i32,
            data: signing_key.verifying_key().as_bytes().to_vec(),
        }
        .encode_to_vec(); // Protobuf encoding

        let multihash = if public_key.len() <= 42 {
            Multihash::wrap(/* Identity */ 0x00, &public_key).expect("Valid Multihash")
        } else {
            let hash = Sha256::new_with_prefix(&public_key).finalize();

            Multihash::wrap(/* Sha256 */ 0x12, &hash).expect("Valid Multihash")
        };

        Cid::new_v1(/* Libp2p key */ 0x72, multihash)
    };

    println!("Addr: {}", addr);

    let signer = Ed25519IPNSRecordSigner { signing_key };

    let record = IPNSRecord::new(value, duration, sequence, ttl, signer).unwrap();

    let raw = record.encode_to_vec();

    let record = IPNSRecord::decode(&*raw).unwrap();

    println!("Record: {:?}", record);

    let result = record.verify(addr);

    println!("Result: {:?}", result);

    assert!(result.is_ok())
}

#[derive(Debug, Signer)]
pub struct Secp256k1Signer {
    signing_key: k256::ecdsa::SigningKey,
}

impl DigestSigner<Sha256, k256::ecdsa::DerSignature> for Secp256k1Signer {
    fn try_sign_digest(&self, digest: Sha256) -> Result<k256::ecdsa::DerSignature, ecdsa::Error> {
        let sig: k256::ecdsa::Signature = self.signing_key.try_sign_digest(digest)?;
        let sig = sig.to_der();

        Ok(sig)
    }
}

impl RecordSigner<k256::ecdsa::DerSignature> for Secp256k1Signer {
    fn crypto_key(&self) -> CryptoKey {
        CryptoKey::new_k256(&self.signing_key.verifying_key())
    }
}

#[test]
fn secp256k1_roundtrip() {
    let value =
        Cid::try_from("bafyreih223c6mqauz5ouolokqrofaekpuu45eblm33fm3g2rlwdkqfabo4").unwrap();

    let duration = Duration::days(30);

    let ttl = 0;
    let sequence = 0;

    use rand_core::OsRng;
    let mut csprng = OsRng {};
    let signing_key: k256::ecdsa::SigningKey = k256::ecdsa::SigningKey::random(&mut csprng);

    let verif_key = k256::ecdsa::VerifyingKey::from(&signing_key);
    let signer = Secp256k1Signer { signing_key };

    let addr = {
        let public_key = CryptoKey {
            r#type: KeyType::Secp256k1 as i32,
            data: verif_key.to_sec1_bytes().to_vec(),
        }
        .encode_to_vec(); // Protobuf encoding

        let multihash = if public_key.len() <= 42 {
            Multihash::wrap(/* Identity */ 0x00, &public_key).expect("Valid Multihash")
        } else {
            let hash = Sha256::new_with_prefix(&public_key).finalize();

            Multihash::wrap(/* Sha256 */ 0x12, &hash).expect("Valid Multihash")
        };

        Cid::new_v1(/* Libp2p key */ 0x72, multihash)
    };

    println!("Addr: {}", addr);

    let record = IPNSRecord::new(value, duration, sequence, ttl, signer).unwrap();

    let raw = record.encode_to_vec();

    let record = IPNSRecord::decode(&*raw).unwrap();

    println!("Record: {:?}", record);

    let result = record.verify(addr);

    println!("Result: {:?}", result);

    assert!(result.is_ok())
}

#[derive(Debug, Signer)]
pub struct EcdsaSigner {
    signing_key: p256::ecdsa::SigningKey,
}

impl DigestSigner<Sha256, p256::ecdsa::DerSignature> for EcdsaSigner {
    fn try_sign_digest(&self, digest: Sha256) -> Result<p256::ecdsa::DerSignature, ecdsa::Error> {
        let sig: p256::ecdsa::Signature = self.signing_key.try_sign_digest(digest)?;
        let sig = sig.to_der();

        Ok(sig)
    }
}

impl RecordSigner<p256::ecdsa::DerSignature> for EcdsaSigner {
    fn crypto_key(&self) -> CryptoKey {
        CryptoKey::new_p256(&self.signing_key.verifying_key())
    }
}

#[test]
fn ecdsa_roundtrip() {
    use elliptic_curve::pkcs8::EncodePublicKey;

    let value =
        Cid::try_from("bafyreih223c6mqauz5ouolokqrofaekpuu45eblm33fm3g2rlwdkqfabo4").unwrap();

    let duration = Duration::days(30);

    let ttl = 0;
    let sequence = 0;

    use rand_core::OsRng;
    let mut csprng = OsRng {};
    let signing_key = p256::ecdsa::SigningKey::random(&mut csprng);

    let verif_key = VerifyingKey::from(&signing_key);
    let signer = EcdsaSigner { signing_key };

    let addr = {
        let public_key = CryptoKey {
            r#type: KeyType::ECDSA as i32,
            data: verif_key
                .to_public_key_der()
                .expect("Valid document")
                .into_vec(),
        }
        .encode_to_vec(); // Protobuf encoding

        let multihash = if public_key.len() <= 42 {
            Multihash::wrap(/* Identity */ 0x00, &public_key).expect("Valid Multihash")
        } else {
            let hash = Sha256::new_with_prefix(&public_key).finalize();

            Multihash::wrap(/* Sha256 */ 0x12, &hash).expect("Valid Multihash")
        };

        Cid::new_v1(/* Libp2p key */ 0x72, multihash)
    };

    println!("Addr: {}", addr);

    let record = IPNSRecord::new(value, duration, sequence, ttl, signer).unwrap();

    let raw = record.encode_to_vec();

    let record = IPNSRecord::decode(&*raw).unwrap();

    println!("Record: {:?}", record);

    let result = record.verify(addr);

    println!("Result: {:?}", result);

    assert!(result.is_ok())
}

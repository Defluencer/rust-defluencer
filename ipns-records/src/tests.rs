#![cfg(test)]

use super::*;

use chrono::Duration;

use ed25519_dalek::{Keypair, PublicKey, SecretKey};

use prost::Message;

use sha2::{Digest, Sha256};

use signature::{DigestSigner, Signer};

use crate::{CryptoKey, IPNSRecord, KeyType, RecordSigner};

pub struct Ed25519IPNSRecordSigner {
    pub keypair: Keypair,
}

impl Signer<ed25519::Signature> for Ed25519IPNSRecordSigner {
    fn sign(&self, msg: &[u8]) -> ed25519::Signature {
        self.try_sign(msg).expect("signature operation failed")
    }

    fn try_sign(&self, msg: &[u8]) -> Result<ed25519::Signature, signature::Error> {
        Ok(self.keypair.sign(msg))
    }
}

impl RecordSigner<ed25519::Signature> for Ed25519IPNSRecordSigner {
    fn crypto_key(&self) -> CryptoKey {
        CryptoKey::new_ed15519_dalek(&self.keypair.public)
    }
}

#[test]
fn ed25519_roundtrip() {
    let value =
        Cid::try_from("bafyreih223c6mqauz5ouolokqrofaekpuu45eblm33fm3g2rlwdkqfabo4").unwrap();

    let duration = Duration::days(30);

    let ttl = 0;
    let sequence = 0;

    // Need rand_core v0.5
    /* use rand_core::OsRng;
    let mut csprng = OsRng {};
    let keypair: Keypair = Keypair::generate(&mut csprng); */

    let secret = SecretKey::from_bytes(&[
        222, 218, 29, 35, 117, 129, 206, 122, 47, 90, 70, 229, 253, 253, 204, 204, 160, 70, 124,
        57, 146, 74, 25, 20, 254, 63, 216, 191, 230, 168, 10, 198,
    ])
    .unwrap();
    let public = PublicKey::from(&secret);
    let keypair = Keypair { secret, public };

    let addr = {
        let public_key = CryptoKey {
            r#type: KeyType::Ed25519 as i32,
            data: keypair.public.to_bytes().to_vec(),
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

    let signer = Ed25519IPNSRecordSigner { keypair };

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

    // Need rand_core v0.6
    /* use rand_core::OsRng;
    let mut csprng = OsRng {};
    let signing_key = k256::ecdsa::SigningKey::random(&mut csprng); */

    let signing_key = k256::ecdsa::SigningKey::from_bytes(&[
        222, 218, 29, 35, 117, 129, 206, 122, 47, 90, 70, 229, 253, 253, 204, 204, 160, 70, 124,
        57, 146, 74, 25, 20, 254, 63, 216, 191, 230, 168, 10, 198,
    ])
    .unwrap();

    let verif_key = signing_key.verifying_key();
    let signer = Secp256k1Signer { signing_key };

    let addr = {
        let public_key = CryptoKey {
            r#type: KeyType::Secp256k1 as i32,
            data: verif_key.to_bytes().to_vec(),
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

    let signing_key = p256::ecdsa::SigningKey::from_bytes(&[
        222, 218, 29, 35, 117, 129, 206, 122, 47, 90, 70, 229, 253, 253, 204, 204, 160, 70, 124,
        57, 146, 74, 25, 20, 254, 63, 216, 191, 230, 168, 10, 198,
    ])
    .unwrap();

    let verif_key = signing_key.verifying_key();
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

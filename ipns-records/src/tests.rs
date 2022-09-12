#![cfg(test)]

use chrono::Duration;

use cid::Cid;

use multihash::Multihash;

use prost::Message;

use sha2::{Digest, Sha256};

use signature::Signer;

use rand_core::OsRng;

use crate::{CryptoKey, IPNSRecord, KeyType};

#[test]
fn ed25519_roundtrip() {
    // Need rand_core v0.5

    use ed25519_dalek::Keypair;

    use signatory::ed25519::{Ed25519Signer, Signature, VerifyingKey};

    pub struct TestSigner {
        pub keypair: Keypair,
    }

    impl Signer<Signature> for TestSigner {
        fn sign(&self, msg: &[u8]) -> Signature {
            self.try_sign(msg).expect("signature operation failed")
        }

        fn try_sign(&self, msg: &[u8]) -> Result<Signature, signature::Error> {
            Ok(self.keypair.sign(msg))
        }
    }

    impl Ed25519Signer for TestSigner {
        fn verifying_key(&self) -> VerifyingKey {
            self.keypair.verifying_key()
        }
    }

    let value =
        Cid::try_from("bafyreih223c6mqauz5ouolokqrofaekpuu45eblm33fm3g2rlwdkqfabo4").unwrap();

    let duration = Duration::days(30);

    let sequence = 0;

    let mut csprng = OsRng {};
    let keypair: Keypair = Keypair::generate(&mut csprng);

    let addr = {
        let public_key = CryptoKey {
            key_type: KeyType::Ed25519 as i32,
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

    let signer = TestSigner { keypair };

    let record = IPNSRecord::new_with_ed25519(value, duration, sequence, signer).unwrap();

    let raw = record.encode_to_vec();

    let record = IPNSRecord::decode(&*raw).unwrap();

    println!("Record: {:?}", record);

    let result = record.verify(addr);

    println!("Result: {:?}", result);

    assert!(result.is_ok())
}

/* #[test]
fn secp256k1_roundtrip() {
    // Need rand_core v0.6

    use k256::ecdsa::{Signature, SigningKey, VerifyingKey};

    pub struct TestSigner {
        pub signing_key: SigningKey,
    }

    impl Signer<Signature> for TestSigner {
        fn sign(&self, msg: &[u8]) -> Signature {
            self.try_sign(msg).expect("signature operation failed")
        }

        fn try_sign(&self, msg: &[u8]) -> Result<Signature, signature::Error> {
            self.signing_key.try_sign(msg)
        }
    }

    impl Secp256k1Signer for TestSigner {
        fn verifying_key(&self) -> VerifyingKey {
            self.signing_key.verifying_key()
        }
    }

    let value =
        Cid::try_from("bafyreih223c6mqauz5ouolokqrofaekpuu45eblm33fm3g2rlwdkqfabo4").unwrap();

    let duration = Duration::days(30);

    let sequence = 0;

    let mut csprng = OsRng {};
    let signing_key = k256::ecdsa::SigningKey::random(&mut csprng);
    let verif_key = signing_key.verifying_key();
    let signer = TestSigner { signing_key };

    let addr = {
        let public_key = CryptoKey {
            key_type: KeyType::Secp256k1 as i32,
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

    let record = IPNSRecord::new_with_secp256k1(value, duration, sequence, signer).unwrap();

    assert_eq!(record.get_address(), addr);

    let raw = record.encode_to_vec();

    let record = IPNSRecord::decode(&*raw).unwrap();

    println!("Record: {:?}", record);

    let result = record.verify(addr);

    assert!(result.is_ok())
} */

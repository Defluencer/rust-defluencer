#![cfg(test)]

use super::*;

use elliptic_curve::sec1::Coordinates;

use signature::Signer;

use crate::JsonWebSignature;

use ed25519_dalek::{Keypair, PublicKey, SecretKey};

pub struct Ed25519Signer {
    pub keypair: Keypair,
}

impl Signer<ed25519::Signature> for Ed25519Signer {
    fn sign(&self, msg: &[u8]) -> ed25519::Signature {
        self.try_sign(msg).expect("signature operation failed")
    }

    fn try_sign(&self, msg: &[u8]) -> Result<ed25519::Signature, signature::Error> {
        Ok(self.keypair.sign(msg))
    }
}

impl BlockSigner<ed25519::Signature> for Ed25519Signer {
    fn algorithm(&self) -> AlgorithmType {
        AlgorithmType::EdDSA
    }

    fn web_key(&self) -> JsonWebKey {
        JsonWebKey {
            key_type: KeyType::OctetString,
            curve: CurveType::Ed25519,
            x: Base::Base64Url.encode(self.keypair.public.to_bytes()),
            y: None,
        }
    }
}

#[test]
fn ed25519_roundtrip() {
    let secret = SecretKey::from_bytes(&[
        222, 218, 29, 35, 117, 129, 206, 122, 47, 90, 70, 229, 253, 253, 204, 204, 160, 70, 124,
        57, 146, 74, 25, 20, 254, 63, 216, 191, 230, 168, 10, 198,
    ])
    .unwrap();
    let public = PublicKey::from(&secret);
    let keypair = Keypair { secret, public };

    // Need rand_core v0.5
    //use rand_core::OsRng;
    //let mut csprng = OsRng {};
    //let keypair: Keypair = Keypair::generate(&mut csprng);

    let signer = Ed25519Signer { keypair };

    let value =
        Cid::try_from("bafyreih223c6mqauz5ouolokqrofaekpuu45eblm33fm3g2rlwdkqfabo4").unwrap();

    let jws = JsonWebSignature::new(value, signer).unwrap();

    let result = jws.verify();

    println!("Result: {:?}", result);

    assert!(result.is_ok())
}

pub struct Secp256k1Signer {
    pub signing_key: k256::ecdsa::SigningKey,
}

impl Signer<k256::ecdsa::Signature> for Secp256k1Signer {
    fn sign(&self, msg: &[u8]) -> k256::ecdsa::Signature {
        self.try_sign(msg).expect("signature operation failed")
    }

    fn try_sign(&self, msg: &[u8]) -> Result<k256::ecdsa::Signature, signature::Error> {
        self.signing_key.try_sign(msg)
    }
}

impl BlockSigner<k256::ecdsa::Signature> for Secp256k1Signer {
    fn algorithm(&self) -> AlgorithmType {
        AlgorithmType::ES256K
    }

    fn web_key(&self) -> JsonWebKey {
        use k256::elliptic_curve::sec1::ToEncodedPoint;

        let verif_key = self.signing_key.verifying_key();
        let point = verif_key.to_encoded_point(false);

        let (x, y) = match point.coordinates() {
            Coordinates::Uncompressed { x, y } => (x, y),
            _ => panic!("Uncompressed Key"),
        };

        JsonWebKey {
            key_type: KeyType::EllipticCurve,
            curve: CurveType::Secp256k1,
            x: Base::Base64Url.encode(x),
            y: Some(Base::Base64Url.encode(y)),
        }
    }
}

#[test]
fn secp256k1_roundtrip() {
    // Need rand_core v0.6
    //use rand_core::OsRng;
    //let mut csprng = OsRng {};
    //let signing_key = SigningKey::random(&mut csprng);

    let signing_key = k256::ecdsa::SigningKey::from_bytes(&[
        222, 218, 29, 35, 117, 129, 206, 122, 47, 90, 70, 229, 253, 253, 204, 204, 160, 70, 124,
        57, 146, 74, 25, 20, 254, 63, 216, 191, 230, 168, 10, 198,
    ])
    .unwrap();

    let signer = Secp256k1Signer { signing_key };

    let value =
        Cid::try_from("bafyreih223c6mqauz5ouolokqrofaekpuu45eblm33fm3g2rlwdkqfabo4").unwrap();

    let jws = JsonWebSignature::new(value, signer).unwrap();

    let result = jws.verify();

    println!("Result: {:?}", result);

    assert!(result.is_ok())
}

pub struct EcdsaSigner {
    pub signing_key: p256::ecdsa::SigningKey,
}

impl Signer<p256::ecdsa::Signature> for EcdsaSigner {
    fn sign(&self, msg: &[u8]) -> p256::ecdsa::Signature {
        self.try_sign(msg).expect("signature operation failed")
    }

    fn try_sign(&self, msg: &[u8]) -> Result<p256::ecdsa::Signature, signature::Error> {
        self.signing_key.try_sign(msg)
    }
}

impl BlockSigner<p256::ecdsa::Signature> for EcdsaSigner {
    fn algorithm(&self) -> AlgorithmType {
        AlgorithmType::ES256
    }

    fn web_key(&self) -> JsonWebKey {
        //use p256::elliptic_curve::sec1::ToEncodedPoint;

        let verif_key = self.signing_key.verifying_key();
        let point = verif_key.to_encoded_point(false);

        let (x, y) = match point.coordinates() {
            Coordinates::Uncompressed { x, y } => (x, y),
            _ => panic!("Uncompressed Key"),
        };

        JsonWebKey {
            key_type: KeyType::EllipticCurve,
            curve: CurveType::P256,
            x: Base::Base64Url.encode(x),
            y: Some(Base::Base64Url.encode(y)),
        }
    }
}

#[test]
fn ecdsa_roundtrip() {
    // Need rand_core v0.6
    //use rand_core::OsRng;
    //let mut csprng = OsRng {};
    //let signing_key = SigningKey::random(&mut csprng);

    let signing_key = p256::ecdsa::SigningKey::from_bytes(&[
        222, 218, 29, 35, 117, 129, 206, 122, 47, 90, 70, 229, 253, 253, 204, 204, 160, 70, 124,
        57, 146, 74, 25, 20, 254, 63, 216, 191, 230, 168, 10, 198,
    ])
    .unwrap();

    let signer = EcdsaSigner { signing_key };

    let value =
        Cid::try_from("bafyreih223c6mqauz5ouolokqrofaekpuu45eblm33fm3g2rlwdkqfabo4").unwrap();

    let jws = JsonWebSignature::new(value, signer).unwrap();

    let result = jws.verify();

    println!("Result: {:?}", result);

    assert!(result.is_ok())
}

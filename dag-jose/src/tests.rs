#![cfg(test)]

use super::*;

use elliptic_curve::sec1::Coordinates;

use signature::Signer;

use crate::JsonWebSignature;

pub struct Ed25519Signer {
    pub signing_key: ed25519_dalek::SigningKey,
}

impl Signer<ed25519::Signature> for Ed25519Signer {
    fn sign(&self, msg: &[u8]) -> ed25519::Signature {
        self.signing_key.sign(msg)
    }

    fn try_sign(&self, msg: &[u8]) -> Result<ed25519::Signature, signature::Error> {
        self.signing_key.try_sign(msg)
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
            x: Base::Base64Url.encode(self.signing_key.verifying_key().as_bytes()),
            y: None,
        }
    }
}

#[test]
fn ed25519_roundtrip() {
    use rand_core::OsRng;

    let mut csprng = OsRng {};
    let signing_key = ed25519_dalek::SigningKey::generate(&mut csprng);

    let signer = Ed25519Signer { signing_key };

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
        self.signing_key.sign(msg)
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
    use rand_core::OsRng;

    let mut csprng = OsRng {};
    let signing_key = k256::ecdsa::SigningKey::random(&mut csprng);
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
        self.signing_key.sign(msg)
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
    use rand_core::OsRng;

    let mut csprng = OsRng {};
    let signing_key = p256::ecdsa::SigningKey::random(&mut csprng);

    let signer = EcdsaSigner { signing_key };

    let value =
        Cid::try_from("bafyreih223c6mqauz5ouolokqrofaekpuu45eblm33fm3g2rlwdkqfabo4").unwrap();

    let jws = JsonWebSignature::new(value, signer).unwrap();

    let result = jws.verify();

    println!("Result: {:?}", result);

    assert!(result.is_ok())
}

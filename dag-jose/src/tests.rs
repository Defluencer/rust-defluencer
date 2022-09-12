#![cfg(test)]

use cid::Cid;

use rand_core::OsRng;

use crate::JsonWebSignature;

/* #[test]
fn ed25519_roundtrip() {
    use ed25519_dalek::{Keypair, Signer};

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

    let mut csprng = OsRng {};
    let keypair: Keypair = Keypair::generate(&mut csprng);// Need rand_core v0.5
    let signer = TestSigner { keypair };

    let value =
        Cid::try_from("bafyreih223c6mqauz5ouolokqrofaekpuu45eblm33fm3g2rlwdkqfabo4").unwrap();

    let jws = JsonWebSignature::new_with_ed25519(value, signer).unwrap();

    let result = jws.verify();

    println!("Result: {:?}", result);

    assert!(result.is_ok())
} */

#[test]
fn secp256k1_roundtrip() {
    use k256::ecdsa::{Signature, SigningKey, VerifyingKey};
    use signatory::ecdsa::Secp256k1Signer;
    use signature::Signer;

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

    let mut csprng = OsRng {};
    let signing_key = SigningKey::random(&mut csprng); // Need rand_core v0.6
    let signer = TestSigner { signing_key };

    let value =
        Cid::try_from("bafyreih223c6mqauz5ouolokqrofaekpuu45eblm33fm3g2rlwdkqfabo4").unwrap();

    let jws = JsonWebSignature::new_with_secp256k1(value, signer).unwrap();

    let result = jws.verify();

    println!("Result: {:?}", result);

    assert!(result.is_ok())
}

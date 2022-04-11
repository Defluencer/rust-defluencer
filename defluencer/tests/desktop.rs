#![cfg(not(target_arch = "wasm32"))]

#[cfg(test)]
mod tests {
    use bip39::{Language, Mnemonic};
    use cid::Cid;

    use defluencer::signatures::{dag_jose::JsonWebSignature, EdDSASigner, Signer};

    use ed25519::KeypairBytes;
    use ipfs_api::IpfsService;

    use linked_data::signature::RawJWS;

    use pkcs8::{EncodePrivateKey, LineEnding};

    use rand_core::{OsRng, RngCore};

    use ed25519_dalek::{Keypair, SecretKey};

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn ed25519_roundtrip() {
        let ipfs = IpfsService::default();

        let mut csprng = OsRng::default();
        let keypair = Keypair::generate(&mut csprng);

        let system = EdDSASigner::new(ipfs.clone(), keypair);

        let cid =
            Cid::try_from("bafybeig6xv5nwphfmvcnektpnojts33jqcuam7bmye2pb54adnrtccjlsu").unwrap();

        let result = system.sign(cid).await.unwrap();

        println!("{}", result);

        let raw: RawJWS = ipfs.dag_get(result, Option::<&str>::None).await.unwrap();

        let jws: JsonWebSignature = raw.try_into().unwrap();

        let result = jws.verify();

        assert!(result.is_ok());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn mnemonic_roundtrip() {
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);

        let secret_key = SecretKey::from_bytes(&bytes).unwrap();

        let key_pair_bytes = KeypairBytes {
            secret_key: secret_key.to_bytes(),
            public_key: None,
        };

        let pem = key_pair_bytes.to_pkcs8_pem(LineEnding::default()).unwrap();

        println!("PEM: {}", pem.to_string());

        let mnemonic = Mnemonic::from_entropy(&bytes, Language::English).unwrap();

        let passphrase = mnemonic.phrase();

        println!("Passphrase: {}", passphrase);

        let mnemonic = Mnemonic::from_phrase(passphrase, Language::English).unwrap();

        assert_eq!(&bytes, mnemonic.entropy());
    }
}

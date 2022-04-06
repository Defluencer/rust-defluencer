#![cfg(not(target_arch = "wasm32"))]

#[cfg(test)]
mod tests {
    use bip39::{Language, Mnemonic};
    use cid::Cid;

    use defluencer::{
        indexing::hamt,
        signatures::{dag_jose::JsonWebSignature, EdDSASigner, Signer},
    };

    use ed25519::KeypairBytes;
    use futures::StreamExt;
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn empty_hamt_get_remove() {
        let ipfs = IpfsService::default();

        // Pre-generated with ipfs.dag_put(&HAMTRoot::default(), Codec::default()).await;
        let index =
            Cid::try_from("bafyreiglvp2q4xij5uzoi7gphdugsbelztsehemnki6hfknqmaitsgblae").unwrap();

        // Random key
        let key =
            Cid::try_from("bafyreiebxcyrgbybcebsk7dwlkidiyi7y6shpvsmneufdouto3pgumvefe").unwrap();

        let result = hamt::get(&ipfs, index.into(), key).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        let result = hamt::remove(&ipfs, index.into(), key).await;

        assert!(result.is_err());
    }

    //TODO use hand crafted hashes so that node fills up instead of spreading.

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn hamt_duplicate_insert() {
        let ipfs = IpfsService::default();

        // Pre-generated with ipfs.dag_put(&HAMTRoot::default(), Codec::default()).await;
        let mut index =
            Cid::try_from("bafyreiglvp2q4xij5uzoi7gphdugsbelztsehemnki6hfknqmaitsgblae").unwrap();

        // Random key
        let key =
            Cid::try_from("bafyreiebxcyrgbybcebsk7dwlkidiyi7y6shpvsmneufdouto3pgumvefe").unwrap();

        // Random value
        let value =
            Cid::try_from("bafyreiejplp7y57dxnasxk7vjdujclpe5hzudiqlgvnit4vinqvtehh3ci").unwrap();

        index = hamt::insert(&ipfs, index.into(), key, value).await.unwrap();

        index = hamt::insert(&ipfs, index.into(), key, value).await.unwrap();

        println!("Hamt Root {}", index);

        let mut stream = hamt::values(&ipfs, index.into()).boxed_local();

        let option = stream.next().await;

        assert!(option.is_some());
        let result = option.unwrap();

        assert!(result.is_ok());
        let cid = result.unwrap();

        assert_eq!(cid, value);

        let option = stream.next().await;

        assert!(option.is_none());
    }
}

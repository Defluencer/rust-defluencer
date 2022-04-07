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

    use multihash::Multihash;
    use pkcs8::{EncodePrivateKey, LineEnding};
    use rand::Rng;
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
        let root =
            Cid::try_from("bafyreif5btv4rgnd443jetidp5iotdh6fdtndhm7c7qtvw32bujcbyk7re").unwrap();

        // Random key
        let key =
            Cid::try_from("bafyreiebxcyrgbybcebsk7dwlkidiyi7y6shpvsmneufdouto3pgumvefe").unwrap();

        let result = hamt::get(&ipfs, root.into(), key).await;

        println!("{:?}", result);

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        let result = hamt::remove(&ipfs, root.into(), key).await;

        println!("{:?}", result);

        assert!(result.is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn hamt_duplicate_insert() {
        let ipfs = IpfsService::default();

        // Pre-generated with ipfs.dag_put(&HAMTRoot::default(), Codec::default()).await;
        let mut root =
            Cid::try_from("bafyreif5btv4rgnd443jetidp5iotdh6fdtndhm7c7qtvw32bujcbyk7re").unwrap();

        // Random key
        let key =
            Cid::try_from("bafyreiebxcyrgbybcebsk7dwlkidiyi7y6shpvsmneufdouto3pgumvefe").unwrap();

        // Random value
        let value =
            Cid::try_from("bafyreiejplp7y57dxnasxk7vjdujclpe5hzudiqlgvnit4vinqvtehh3ci").unwrap();

        root = hamt::insert(&ipfs, root.into(), key, value).await.unwrap();

        root = hamt::insert(&ipfs, root.into(), key, value).await.unwrap();

        println!("Root {}", root);

        let mut stream = hamt::values(&ipfs, root.into()).boxed_local();

        let option = stream.next().await;

        assert!(option.is_some());
        let result = option.unwrap();

        assert!(result.is_ok());
        let cid = result.unwrap();

        assert_eq!(cid, value);

        let option = stream.next().await;

        assert!(option.is_none());
    }

    use rand_xoshiro::{rand_core::SeedableRng, Xoshiro256StarStar};

    fn random_cid(rng: &mut Xoshiro256StarStar) -> Cid {
        let mut hash = [0u8; 32];
        rng.fill_bytes(&mut hash);

        let multihash = Multihash::wrap(0x12, &hash).unwrap();
        let cid = Cid::new_v1(0x71, multihash);

        cid
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn hamt_linear_insert() {
        let ipfs = IpfsService::default();

        let mut rng = Xoshiro256StarStar::seed_from_u64(2347867832489023);

        // Pre-generated with ipfs.dag_put(&HAMTRoot::default(), Codec::default()).await;
        let mut root =
            Cid::try_from("bafyreif5btv4rgnd443jetidp5iotdh6fdtndhm7c7qtvw32bujcbyk7re").unwrap();

        let count = 256;

        for _ in 0..count {
            let key = random_cid(&mut rng);
            let value = key;

            let result = hamt::insert(&ipfs, root.into(), key, value).await;

            match result {
                Ok(cid) => (root = cid),
                Err(e) => panic!("Index: {} Key: {} Error: {}", root, key, e),
            }
        }

        let sum = hamt::values(&ipfs, root.into())
            .fold(0, |acc, _| async move { acc + 1 })
            .await;

        assert_eq!(count, sum);

        println!("Root {}", root);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn hamt_remove_collapse() {
        let ipfs = IpfsService::default();

        // Pre-generated with hamt_random_insert;
        let mut root =
            Cid::try_from("bafyreicdmpbc23de3n5o6lu7qr2nnzn2dv4a7ulz6k2ouwzsrsctnmbcta").unwrap();

        let key =
            Cid::try_from("bafyreiarw4llrjyv6ctuhyupx65tzbgr37kkiyjwyxj6blnmekpfx32ysu").unwrap();

        let result = hamt::remove(&ipfs, root.into(), key).await;

        match result {
            Ok(cid) => {
                root = cid;
            }
            Err(e) => panic!("Root: {} Key: {} Error: {}", root, key, e),
        }

        let key =
            Cid::try_from("bafyreiark2h2b2yumkvhzqttaw66eyu4benkpbyk34qwokj6s6ftafxl6m").unwrap();

        let result = hamt::remove(&ipfs, root.into(), key).await;

        match result {
            Ok(cid) => println!("Root: {}", cid),
            Err(e) => panic!("Root: {} Key: {} Error: {}", root, key, e),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn hamt_linear_remove() {
        let ipfs = IpfsService::default();

        let mut rng = Xoshiro256StarStar::seed_from_u64(2347867832489023);

        // Pre-generated with hamt_random_insert;
        let mut root =
            Cid::try_from("bafyreicdmpbc23de3n5o6lu7qr2nnzn2dv4a7ulz6k2ouwzsrsctnmbcta").unwrap();

        for _ in 0..256 {
            let key = random_cid(&mut rng);

            let result = hamt::remove(&ipfs, root.into(), key).await;

            match result {
                Ok(cid) => (root = cid),
                Err(e) => panic!("Root: {} Key: {} Error: {}", root, key, e),
            }
        }

        let sum = hamt::values(&ipfs, root.into())
            .fold(0, |acc, _| async move { acc + 1 })
            .await;

        assert_eq!(0, sum);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn hamt_fuzzy() {
        let ipfs = IpfsService::default();

        let mut rng = Xoshiro256StarStar::seed_from_u64(2347867832489023);

        // Pre-generated with ipfs.dag_put(&HAMTRoot::default(), Codec::default()).await;
        let mut root =
            Cid::try_from("bafyreif5btv4rgnd443jetidp5iotdh6fdtndhm7c7qtvw32bujcbyk7re").unwrap();

        let count = 500;

        let mut keys = Vec::with_capacity(count);

        for _ in 0..count {
            if keys.is_empty() || rng.gen_ratio(2, 3) {
                let key = random_cid(&mut rng);
                let value = key;

                keys.push(key);

                let result = hamt::insert(&ipfs, root.into(), key, value).await;

                match result {
                    Ok(cid) => (root = cid),
                    Err(e) => panic!("Index: {} Key: {} Error: {}", root, key, e),
                }
            } else {
                let idx = rng.gen_range(0..keys.len());

                let key = keys.remove(idx);

                let result = hamt::remove(&ipfs, root.into(), key).await;

                match result {
                    Ok(cid) => (root = cid),
                    Err(e) => panic!("Root: {} Key: {} Error: {}", root, key, e),
                }
            }
        }

        let sum = hamt::values(&ipfs, root.into())
            .fold(0, |acc, _| async move { acc + 1 })
            .await;

        println!("Final Count {} Root {}", sum, root);
    }
}

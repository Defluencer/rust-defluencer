#![cfg(not(target_arch = "wasm32"))]

#[cfg(test)]
mod tests {
    /* use chrono::{Duration, SecondsFormat, Utc};

    use cid::Cid;
    use sha3::Keccak256;

    use std::ops::Add;

    use ipfs_api::IpfsService;

    use linked_data::types::IPNSAddress;

    use multihash::Multihash;

    use rand_core::OsRng; */

    /* #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn ed25519_roundtrip() {
        let ipfs = IpfsService::default();

        let mut csprng = OsRng::default();
        let keypair = ed25519_dalek::Keypair::generate(&mut csprng);

        let system = EdDSASigner::new(ipfs.clone(), keypair);

        let cid =
            Cid::try_from("bafybeig6xv5nwphfmvcnektpnojts33jqcuam7bmye2pb54adnrtccjlsu").unwrap();

        let result = system.sign(cid).await.unwrap();

        println!("{}", result);

        let raw: RawJWS = ipfs.dag_get(result, Option::<&str>::None).await.unwrap();

        let jws: JsonWebSignature = raw.try_into().unwrap();

        let result = jws.verify();

        assert!(result.is_ok());
    } */

    /* #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn mnemonic_roundtrip() {
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);

        let secret_key = ed25519_dalek::SecretKey::from_bytes(&bytes).unwrap();

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
    } */

    /* #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn ipns_sub() {
        use futures::StreamExt;

        let defluencer = Defluencer::default();

        let ipns = IPNSAddress::try_from(
            "bafzaajaiaejcbzhovvpbohh2fjeosmfkak45n4hilt5wcxnum4btp5ztxyktac6r",
        )
        .unwrap();

        let (_handle, regis) = AbortHandle::new_pair();

        let mut stream = defluencer.subscribe_ipns_updates(ipns, regis).boxed_local();

        let cid = stream.next().await.unwrap().unwrap();

        println!("Current {}", cid);

        let cid = stream.next().await.unwrap().unwrap();

        println!("New {}", cid);
    } */

    /* #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn ipns_esoteric_record() {
        use prost::Message;

        let cid =
            Cid::try_from("bafyreieoextjee6sm5hpaxdhkseypz3by6vzcwddhxa673ozmxxjwv3hv4").unwrap();
        let value = format!("/ipfs/{}", cid.to_string()).into_bytes();

        let validity = Utc::now()
            .add(Duration::weeks(1))
            .to_rfc3339_opts(SecondsFormat::Nanos, false)
            .into_bytes();

        let validity_type = ValidityType::EOL;

        let mut csprng = OsRng::default();

        let signing_key = k256::ecdsa::SigningKey::random(&mut csprng);
        let verifying_key = signing_key.verifying_key();

        let signature = {
            let mut signing_input = Vec::with_capacity(
                value.len() + validity.len() + 3, /* b"EOL".len() == 3 */
            );

            signing_input.extend(value.iter());
            signing_input.extend(validity.iter());
            signing_input.extend(validity_type.to_string().as_bytes());

            use sha3::Digest;
            use signature::DigestSigner;
            let digest = Keccak256::new_with_prefix(signing_input);
            // IPNS verify record with sha2-256 signed message but ETH use Keccak256
            let signature: k256::ecdsa::Signature = signing_key.sign_digest(digest);

            signature.to_der().to_bytes().into_vec()
        };

        let public_key = {
            //let key = k256::PublicKey::from(verifying_key);

            let data = verifying_key.to_bytes().to_vec();

            let key = CryptoKey {
                key_type: KeyType::Secp256k1 as i32,
                data,
            };

            key.encode_to_vec() // Protobuf encoding
        };

        let address = {
            let multihash = Multihash::wrap(0x00, &public_key).unwrap();

            Cid::new_v1(0x72, multihash)
        };

        let record = IPNSRecord {
            value,
            signature,
            validity_type: validity_type as i32,
            validity,
            sequence: 1,
            ttl: 0,
            public_key,
        };

        let record_data = record.encode_to_vec(); // Protobuf encoding

        let ipfs = IpfsService::default();

        let response = ipfs.dht_put(address, record_data).await;

        let ipns: IPNSAddress = address.into();

        println!(
            "IPNS Address: {}\nPubsub Topic: {}\nResolve: {}\nResponse: {:#?}",
            ipns,
            ipns.to_pubsub_topic(),
            cid,
            response,
        );
    } */

    //btc
    // identity bafyreickdtfrlbpowjxj3ovqnr6ys2jaqquwai2osmuhasxd6y527irf6m
    // channel bafzaajaiaejcbisrjbhnlu2sbp7q5spgi4caua7j52bwpeszw5fpxxnq3nyrhmuk
    // microblog bafyreiawmuaooj2qevyns2ni3yoa7iszngppcug3u6udr2oarlkklxcjay
    // comment bafyreib3zu6mdx4fysfu3e5byzwnyglylwo5i5lee7lf6zvr7ifr55bboa

    // root bafyreihkvkha7spbsrbkl4eyx6hjbpaxreutdsll6aisce53m5at2rnry4
}

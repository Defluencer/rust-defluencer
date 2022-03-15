#![cfg(not(target_arch = "wasm32"))]

#[cfg(test)]
mod tests {
    use cid::Cid;

    use defluencer::signatures::{dag_jose::JsonWebSignature, EdDSASigner, Signer};

    use ipfs_api::IpfsService;

    use linked_data::signature::RawJWS;

    use rand_core::OsRng;

    use ed25519_dalek::Keypair;

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
}

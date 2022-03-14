#![cfg(not(target_arch = "wasm32"))]

#[cfg(test)]
mod tests {
    use cid::Cid;

    use defluencer::signature_system::{dag_jose::verify_jws, IPNSSignature, SignatureSystem};

    use ipfs_api::IpfsService;

    use linked_data::signature::RawJWS;

    use rand_core::OsRng;

    use ed25519_dalek::Keypair;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn ed25519_roundtrip() {
        let ipfs = IpfsService::default();

        let mut csprng = OsRng::default();
        let keypair = Keypair::generate(&mut csprng);

        let system = IPNSSignature::new(ipfs.clone(), keypair);

        let cid =
            Cid::try_from("bafybeig6xv5nwphfmvcnektpnojts33jqcuam7bmye2pb54adnrtccjlsu").unwrap();

        let result = system.sign(cid).await.unwrap();

        println!("{}", result);

        let raw: RawJWS = ipfs.dag_get(result, Option::<&str>::None).await.unwrap();

        let result = verify_jws(raw);

        assert!(result.is_ok());
    }
}

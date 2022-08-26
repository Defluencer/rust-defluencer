use async_trait::async_trait;

use multihash::Multihash;

use prost::Message;

use sha2::Digest;

/// IPNS updater using Ledger nano app to create records.
#[derive(Clone)]
pub struct LedgerNanoUpdater {
    ipfs: IpfsService,
    signer: IpfsNanoApp, //TODO build a ledger nano app that can create IPNS records
}

#[async_trait(?Send)]
impl IpnsUpdater for LedgerNanoUpdater {
    async fn update(&self, cid: Cid) -> Result<(), Error> {
        let value = format!("/ipfs/{}", cid.to_string()).into_bytes();

        let validity = Utc::now()
            .add(Duration::weeks(52))
            .to_rfc3339_opts(SecondsFormat::Nanos, false)
            .into_bytes();

        let validity_type = ValidityType::EOL;

        let signing_input = {
            let mut data = Vec::with_capacity(
                value.len() + validity.len() + 3, /* b"EOL".len() == 3 */
            );

            data.extend(value.iter());
            data.extend(validity.iter());
            data.extend(validity_type.to_string().as_bytes());

            data
        };

        let (public_key, signature) = self.signer.sign(&signing_input).await?;

        let verifying_key = k256::ecdsa::VerifyingKey::from(public_key);
        let signature = signature.to_der().to_bytes().into_vec();

        let public_key = CryptoKey {
            key_type: KeyType::Secp256k1 as i32,
            data: verifying_key.to_bytes().to_vec(),
        }
        .encode_to_vec(); // Protobuf encoding

        let ipns = {
            let multihash = if public_key.len() <= 42 {
                Multihash::wrap(0x00, &public_key).unwrap()
            } else {
                let hash = sha2::Sha256::new_with_prefix(&public_key).finalize();

                Multihash::wrap(0x12, &hash).unwrap()
            };

            Cid::new_v1(0x72, multihash)
        };

        let record = IPNSRecord {
            value,
            signature,
            validity_type: validity_type as i32,
            validity,
            sequence,
            ttl: 0, //TODO figure this out!
            public_key,
        };

        let record_data = record.encode_to_vec(); // Protobuf encoding

        self.ipfs.dht_put(ipns, record_data).await?;

        Ok(())
    }
}

#![cfg(not(target_arch = "wasm32"))]

use async_trait::async_trait;

use cid::Cid;

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::signature::{
    AlgorithmType, CurveType, Header, JsonWebKey, KeyType, RawJWS, RawSignature,
};

use multibase::Base;

use crate::errors::Error;

use ed25519::signature::Signature;

use ed25519_dalek::{Keypair, Signer};

/// Create DAG-Jose blocks with the EdDSA.
pub struct EdDSASigner {
    ipfs: IpfsService,
    key_pair: Keypair,
}

impl EdDSASigner {
    pub fn new(ipfs: IpfsService, key_pair: Keypair) -> Self {
        Self { ipfs, key_pair }
    }
}

#[async_trait(?Send)]
impl super::Signer for EdDSASigner {
    /// Create a DAG-JOSE block linked to the input CID.
    ///
    /// Returns the block CID.
    async fn sign(&self, cid: Cid) -> Result<Cid, Error> {
        let header = Some(Header {
            algorithm: None,
            json_web_key: Some(JsonWebKey {
                key_type: KeyType::OctetString,
                curve: CurveType::Ed25519,
                x: Base::Base64Url.encode(self.key_pair.public.as_bytes()),
                y: None,
            }),
        });

        let payload = cid.to_bytes();
        let payload = Base::Base64Url.encode(payload);

        let protected = Header {
            algorithm: Some(AlgorithmType::EdDSA),
            json_web_key: None,
        };
        let protected = serde_json::to_vec(&protected).unwrap();
        let protected = Base::Base64Url.encode(protected);

        let signing_input = format!("{}.{}", payload, protected);

        let signature = self.key_pair.sign(signing_input.as_bytes());
        let signature = Base::Base64Url.encode(signature.as_bytes());

        let json = RawJWS {
            payload,
            signatures: vec![RawSignature {
                header,
                protected,
                signature,
            }],
            link: cid.into(),
        };

        let cid = self.ipfs.dag_put(&json, Codec::DagJose).await?;

        Ok(cid)
    }
}

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

/// Signature systems are responsable for signing & authenticating content.
#[async_trait(?Send)]
pub trait SignatureSystem {
    /// Create a DAG-JOSE block linking to this cid.
    async fn sign(&self, cid: Cid) -> Result<Cid, Error>;
}

pub struct IPNSSignature {
    ipfs: IpfsService,
    key_pair: Keypair,
}

impl IPNSSignature {
    pub fn new(ipfs: IpfsService, key_pair: Keypair) -> Self {
        Self { ipfs, key_pair }
    }
}

#[async_trait(?Send)]
impl SignatureSystem for IPNSSignature {
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

#[cfg(target_arch = "wasm32")]
use web3::{transports::eip_1193::Eip1193, types::Address, Web3};

#[cfg(target_arch = "wasm32")]
pub struct ENSSignature {
    ipfs: IpfsService,
    addr: Address,
    web3: Web3<Eip1193>,
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl SignatureSystem for ENSSignature {
    async fn sign(&self, cid: Cid) -> Result<Cid, Error> {
        let payload = cid.to_bytes();
        let payload = Base::Base64Url.encode(payload);

        let protected = Header {
            algorithm: Some(AlgorithmType::ES256K),
            json_web_key: None,
        };

        let protected = serde_json::to_vec(&protected).unwrap();
        let protected = Base::Base64Url.encode(protected);

        let message = format!("{}.{}", payload, protected);

        let sig = self
            .web3
            .personal()
            .sign(message.clone().into(), self.addr, "")
            .await?;
        let signature = sig.to_fixed_bytes();

        let jwk = k256_recover(message.as_bytes(), &signature)?;

        let header = Some(Header {
            algorithm: None,
            json_web_key: Some(jwk),
        });

        let signature = Base::Base64Url.encode(signature);

        let json = RawJWS {
            payload,
            signatures: vec![RawSignature {
                header,
                protected,
                signature,
            }],
            link: cid.into(), // ignored when serializing
        };

        let cid = self.ipfs.dag_put(&json, Codec::DagJose).await?;

        Ok(cid)
    }
}

#[cfg(target_arch = "wasm32")]
fn k256_recover(message: &[u8], signature: &[u8]) -> Result<JsonWebKey, Error> {
    let mut eth_message = format!("\x19Ethereum Signed Message:\n{}", message.len()).into_bytes();
    eth_message.extend_from_slice(&message);

    let sig = k256::ecdsa::recoverable::Signature::from_bytes(signature)?;

    let recovered_key = sig.recover_verify_key(&eth_message)?; // The fn hash the message
    let public_key = k256::PublicKey::from(recovered_key);

    // Lazy Hack: Deserialize then serialize as the other type
    let jwk_string = public_key.to_jwk_string();
    let jwk: JsonWebKey = serde_json::from_str(&jwk_string)?;

    Ok(jwk)
}

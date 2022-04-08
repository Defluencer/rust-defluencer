#![cfg(target_arch = "wasm32")]

use async_trait::async_trait;

use cid::Cid;

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::signature::{
    AlgorithmType,  Header, JsonWebKey,  RawJWS, RawSignature,
};

use multibase::Base;

use crate::errors::Error;

use web3::{transports::eip_1193::Eip1193, types::Address, Web3};

use signature::Signature;

/// Create DAG-Jose blocks with the EcDSA.
pub struct EthereumSigner {
    ipfs: IpfsService,
    addr: Address,
    web3: Web3<Eip1193>,
}

impl EthereumSigner {
    pub fn new(ipfs: IpfsService, addr: Address, web3: Web3<Eip1193>) -> Self {
        Self { ipfs, addr, web3 }
    }
}

#[async_trait(?Send)]
impl super::Signer for EthereumSigner {
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
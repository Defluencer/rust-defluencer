#![cfg(target_arch = "wasm32")]

use async_trait::async_trait;

use linked_data::types::Address;

use crate::errors::Error;

use web3::{transports::eip_1193::Eip1193, Web3};

use signature::Signature;

#[derive(Clone)]
pub struct EthereumWebSigner {
    addr: Address,
    web3: Web3<Eip1193>,
}

impl EthereumWebSigner {
    pub fn new( addr: Address, web3: Web3<Eip1193>) -> Self {
        Self { addr, web3 }
    }
}

#[async_trait(?Send)]
impl super::Signer for EthereumWebSigner {
    async fn sign(&self, signing_input: Vec<u8>) -> Result<(k256::PublicKey, k256::ecdsa::Signature), Error> {
        let mut eth_message =
            format!("\x19Ethereum Signed Message:\n{}", signing_input.len()).into_bytes();
        eth_message.extend_from_slice(&signing_input);

        let sig = self
            .web3
            .personal()
            .sign(signing_input.into(), self.addr.into(), "")
            .await?;
        
        let signature = k256::ecdsa::recoverable::Signature::from_bytes(&sig.to_fixed_bytes())?;
        
        let recovered_key = signature.recover_verify_key(&eth_message)?; // The fn hash the message
        
        let public_key = k256::PublicKey::from(recovered_key);
        let signature = k256::ecdsa::Signature::from(signature);

        Ok((public_key, signature))
    }
}

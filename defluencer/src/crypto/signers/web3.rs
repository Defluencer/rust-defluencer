#![cfg(target_arch = "wasm32")]

use async_trait::async_trait;

use sha3::{Digest, Keccak256};

use crate::{crypto::signed_link::HashAlgorithm, errors::Error};

use super::Signer;

use web3::{transports::eip_1193::Eip1193, Web3};

use linked_data::types::Address;

#[derive(Clone)]
pub struct Web3Signer {
    addr: Address,
    web3: Web3<Eip1193>,
}

impl Web3Signer {
    pub fn new(addr: Address, web3: Web3<Eip1193>) -> Self {
        Self { addr, web3 }
    }
}

#[async_trait(?Send)]
impl Signer for Web3Signer {
    async fn sign(
        &self,
        signing_input: &[u8],
    ) -> Result<
        (
            k256::ecdsa::VerifyingKey,
            k256::ecdsa::Signature,
            HashAlgorithm,
        ),
        Error,
    > {
        use k256::ecdsa::signature::Signature;

        let sig = self
            .web3
            .personal()
            .sign(signing_input.into(), self.addr.into(), "")
            .await?;

        // The k256 crate expect 0 OR 1 as recovery ID, instead Metamask return 27 OR 28
        let mut bytes = sig.to_fixed_bytes();
        if bytes[64] == 27 || bytes[64] == 28 {
            bytes[64] -= 27;
        }

        let signature = k256::ecdsa::recoverable::Signature::from_bytes(&bytes)?;

        let mut eth_message =
            format!("\x19Ethereum Signed Message:\n{}", signing_input.len()).into_bytes();
        eth_message.extend_from_slice(signing_input);

        let digest = Keccak256::new_with_prefix(eth_message);

        let recovered_key = signature.recover_verifying_key_from_digest(digest)?;

        let signature = k256::ecdsa::Signature::from(signature);

        Ok((recovered_key, signature, HashAlgorithm::EthereumLedgerApp))
    }
}

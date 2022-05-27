use async_trait::async_trait;

use crate::errors::Error;

#[cfg(not(target_arch = "wasm32"))]
use super::ledger::EthereumLedgerApp;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
pub struct EthereumSigner {
    app: EthereumLedgerApp,
    account_index: u32,
}

#[cfg(not(target_arch = "wasm32"))]
impl EthereumSigner {
    pub fn new(app: EthereumLedgerApp, account_index: u32) -> Self {
        Self { app, account_index }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait(?Send)]
impl super::Signer for EthereumSigner {
    async fn sign(
        &self,
        signing_input: Vec<u8>,
    ) -> Result<(k256::PublicKey, k256::ecdsa::Signature), Error> {
        use signature::Verifier;

        let mut eth_message =
            format!("\x19Ethereum Signed Message:\n{}", signing_input.len()).into_bytes();
        eth_message.extend_from_slice(&signing_input);

        let signature = self
            .app
            .sign_personal_message(&signing_input, self.account_index)?;

        let recovered_key = signature.recover_verifying_key(&eth_message)?; // The fn hash the message

        recovered_key.verify(&eth_message, &signature)?;

        let public_key = k256::PublicKey::from(recovered_key);
        let signature = k256::ecdsa::Signature::from(signature);

        Ok((public_key, signature))
    }
}

#[cfg(target_arch = "wasm32")]
use web3::{transports::eip_1193::Eip1193, Web3};

#[cfg(target_arch = "wasm32")]
use linked_data::types::Address;

#[cfg(target_arch = "wasm32")]
#[derive(Clone)]
pub struct EthereumSigner {
    addr: Address,
    web3: Web3<Eip1193>,
}

#[cfg(target_arch = "wasm32")]
impl EthereumSigner {
    pub fn new(addr: Address, web3: Web3<Eip1193>) -> Self {
        Self { addr, web3 }
    }
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl super::Signer for EthereumSigner {
    async fn sign(
        &self,
        signing_input: Vec<u8>,
    ) -> Result<(k256::PublicKey, k256::ecdsa::Signature), Error> {
        use signature::Signature;
        use signature::Verifier;

        let mut eth_message =
            format!("\x19Ethereum Signed Message:\n{}", signing_input.len()).into_bytes();
        eth_message.extend_from_slice(&signing_input);

        let sig = self
            .web3
            .personal()
            .sign(signing_input.into(), self.addr.into(), "")
            .await?;

        let signature = k256::ecdsa::recoverable::Signature::from_bytes(&sig.to_fixed_bytes())?;

        let recovered_key = signature.recover_verifying_key(&eth_message)?; // The fn hash the message

        recovered_key.verify(&eth_message, &signature)?;

        let public_key = k256::PublicKey::from(recovered_key);
        let signature = k256::ecdsa::Signature::from(signature);

        Ok((public_key, signature))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use sha2::{Digest, Sha256};
    //use sha3::{Digest, Keccak256};

    #[test]
    fn sign() {
        use signature::Verifier;

        let app = EthereumLedgerApp::default();

        let signing_input = b"Hello World!";

        let mut eth_message =
            format!("\x19Ethereum Signed Message:\n{}", signing_input.len()).into_bytes();
        eth_message.extend_from_slice(signing_input);

        let mut hasher = Sha256::new();
        hasher.update(signing_input.clone());
        let hash = hasher.finalize();

        println!("Hash: {}", hex::encode(hash));

        let signature = app.sign_personal_message(signing_input, 0).unwrap();

        let recovered_key = signature.recover_verifying_key(&eth_message).unwrap(); // The fn hash the message

        recovered_key.verify(&eth_message, &signature).unwrap();
    }

    #[test]
    fn addr() {
        let app = EthereumLedgerApp::default();

        let (_public_key, addr) = app.get_public_address(0).expect("Get Address Result");

        println!("Address: {}", addr);
    }
}

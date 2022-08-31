use async_trait::async_trait;

use crate::errors::Error;

use sha3::{Digest, Keccak256};

use super::signed_link::HashAlgorithm;

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

    pub fn get_public_address(&self) -> Result<String, Error> {
        let (_, addr) = self.app.get_public_address(self.account_index)?;

        Ok(addr)
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait(?Send)]
impl super::Signer for EthereumSigner {
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
        let signature = self
            .app
            .sign_personal_message(signing_input, self.account_index)?;

        let mut eth_message =
            format!("\x19Ethereum Signed Message:\n{}", signing_input.len()).into_bytes();
        eth_message.extend_from_slice(signing_input);

        let digest = Keccak256::new_with_prefix(eth_message);

        let recovered_key = signature.recover_verifying_key_from_digest(digest)?;

        let signature = k256::ecdsa::Signature::from(signature);

        Ok((recovered_key, signature, HashAlgorithm::EthereumLedgerApp))
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
        signing_input: &[u8],
    ) -> Result<
        (
            k256::ecdsa::VerifyingKey,
            k256::ecdsa::Signature,
            HashAlgorithm,
        ),
        Error,
    > {
        use signature::Signature;

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

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use super::*;

    use k256::ecdsa::VerifyingKey;

    use sha2::Digest;

    use signature::DigestVerifier;

    #[test]
    fn sign_test() {
        let app = EthereumLedgerApp::default();
        let account_index = 0;

        let (pub_key, _account) = app.get_public_address(account_index).unwrap();
        let verif_key = VerifyingKey::from(pub_key);

        //let signing_input = b"Hello World!";
        let signing_input = b"The root problem with conventional currency is all the trust that's required to make it work. The central bank must be trusted not to debase the currency, but the history of fiat currencies is full of breaches of that trust. Banks must be trusted to hold our money and transfer it electronically, but they lend it out in waves of credit bubbles with barely a fraction in reserve. We have to trust them with our privacy, trust them not to let identity thieves drain our accounts. Their massive overhead costs make micropayments impossible.";
        //let signing_input = &[255_u8; 85];

        let signature = app
            .sign_personal_message(signing_input, account_index)
            .unwrap();

        let mut eth_message =
            format!("\x19Ethereum Signed Message:\n{}", signing_input.len()).into_bytes();
        eth_message.extend_from_slice(signing_input);

        let digest = Keccak256::new_with_prefix(eth_message);

        let recovered_key = signature
            .recover_verifying_key_from_digest(digest.clone())
            .unwrap();

        assert_eq!(recovered_key, verif_key);

        verif_key.verify_digest(digest, &signature).unwrap();
    }

    #[test]
    fn addr() {
        let app = EthereumLedgerApp::default();

        let (_public_key, addr) = app.get_public_address(0).expect("Get Address Result");

        println!("Address: {}", addr);
    }
}

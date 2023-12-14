#![cfg(not(target_arch = "wasm32"))]

use async_trait::async_trait;

use sha3::{Digest, Keccak256};

use k256::ecdsa::{Signature, VerifyingKey};

use crate::crypto::ledger::EthereumLedgerApp;

use crate::{crypto::signed_link::HashAlgorithm, errors::Error};

use super::Signer;

#[derive(Clone)]
pub struct EthereumSigner {
    app: EthereumLedgerApp,
    account_index: u32,
}

impl EthereumSigner {
    pub fn new(app: EthereumLedgerApp, account_index: u32) -> Self {
        Self { app, account_index }
    }

    pub fn get_public_address(&self) -> Result<String, Error> {
        let (_, addr) = self.app.get_public_address(self.account_index)?;

        Ok(addr)
    }
}

#[async_trait(?Send)]
impl Signer for EthereumSigner {
    async fn sign(
        &self,
        signing_input: &[u8],
    ) -> Result<(VerifyingKey, Signature, HashAlgorithm), Error> {
        let (signature, rec_id) = self
            .app
            .sign_personal_message(signing_input, self.account_index)?;

        let mut eth_message =
            format!("\x19Ethereum Signed Message:\n{}", signing_input.len()).into_bytes();
        eth_message.extend_from_slice(signing_input);

        let digest = Keccak256::new_with_prefix(eth_message);

        let recovered_key = VerifyingKey::recover_from_digest(digest, &signature, rec_id)?;

        Ok((recovered_key, signature, HashAlgorithm::EthereumLedgerApp))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn sign_test() {
        use k256::ecdsa::signature::DigestVerifier;

        let app = EthereumLedgerApp::default();
        let account_index = 0;

        let (pub_key, _account) = app.get_public_address(account_index).unwrap();
        let verif_key = VerifyingKey::from(pub_key);

        //let signing_input = b"Hello World!";
        let signing_input = b"The root problem with conventional currency is all the trust that's required to make it work. The central bank must be trusted not to debase the currency, but the history of fiat currencies is full of breaches of that trust. Banks must be trusted to hold our money and transfer it electronically, but they lend it out in waves of credit bubbles with barely a fraction in reserve. We have to trust them with our privacy, trust them not to let identity thieves drain our accounts. Their massive overhead costs make micropayments impossible.";
        //let signing_input = &[255_u8; 85];

        let (signature, rec_id) = app
            .sign_personal_message(signing_input, account_index)
            .unwrap();

        let mut eth_message =
            format!("\x19Ethereum Signed Message:\n{}", signing_input.len()).into_bytes();
        eth_message.extend_from_slice(signing_input);

        let digest = Keccak256::new_with_prefix(eth_message);

        let recovered_key =
            VerifyingKey::recover_from_digest(digest.clone(), &signature, rec_id).unwrap();

        assert_eq!(recovered_key, verif_key);

        verif_key.verify_digest(digest, &signature).unwrap();
    }

    #[test]
    #[ignore]
    fn addr() {
        let app = EthereumLedgerApp::default();

        let (_public_key, addr) = app.get_public_address(0).expect("Get Address Result");

        println!("Address: {}", addr);
    }
}

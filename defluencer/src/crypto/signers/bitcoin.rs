#![cfg(not(target_arch = "wasm32"))]

use async_trait::async_trait;

use sha2::{Digest, Sha256};

use k256::ecdsa::{Signature, VerifyingKey};

use crate::{
    crypto::{ledger::BitcoinLedgerApp, signed_link::HashAlgorithm},
    errors::Error,
    utils::VarInt,
};

use super::Signer;

#[derive(Clone)]
pub struct BitcoinSigner {
    app: BitcoinLedgerApp,
    account_index: u32,
}

impl BitcoinSigner {
    pub fn new(app: BitcoinLedgerApp, account_index: u32) -> Self {
        Self { app, account_index }
    }

    pub fn get_public_address(&self) -> Result<String, Error> {
        let (_, addr) = self.app.get_extended_pubkey(self.account_index)?;

        Ok(addr)
    }
}

#[async_trait(?Send)]
impl Signer for BitcoinSigner {
    async fn sign(
        &self,
        signing_input: &[u8],
    ) -> Result<(VerifyingKey, Signature, HashAlgorithm), Error> {
        let (signature, rec_id) = self.app.sign_message(signing_input, self.account_index)?;

        let btc_message = {
            let mut temp = Vec::from("\x18Bitcoin Signed Message:\n");

            let msg_len = VarInt(signing_input.len() as u64).consensus_encode();

            temp.extend(&msg_len);
            temp.extend(signing_input);
            temp
        };

        let hash = Sha256::new_with_prefix(btc_message).finalize();
        let digest = Sha256::new_with_prefix(hash);
        let recovered_key = VerifyingKey::recover_from_digest(digest, &signature, rec_id)?;

        Ok((recovered_key, signature, HashAlgorithm::BitcoinLedgerApp))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use k256::ecdsa::signature::DigestVerifier;

    use sha2::Digest;

    #[test]
    #[ignore]
    fn addr() {
        let app = BitcoinLedgerApp::default();

        let (_, addr) = app.get_extended_pubkey(0).unwrap();

        println!("Address: {}", addr);
    }

    #[test]
    #[ignore]
    fn sign_test() {
        let app = BitcoinLedgerApp::default();
        let account_index = 0;

        let (pub_key, _addr) = app.get_extended_pubkey(account_index).unwrap();
        let verif_key = VerifyingKey::from(pub_key);

        //let signing_input = b"Hello World!";
        let signing_input = b"The root problem with conventional currency is all the trust that's required to make it work. The central bank must be trusted not to debase the currency, but the history of fiat currencies is full of breaches of that trust. Banks must be trusted to hold our money and transfer it electronically, but they lend it out in waves of credit bubbles with barely a fraction in reserve. We have to trust them with our privacy, trust them not to let identity thieves drain our accounts. Their massive overhead costs make micropayments impossible.";

        let display_hash = Sha256::new_with_prefix(signing_input).finalize();
        println!("Message Display Hash: 0x{}", hex::encode(display_hash));

        let (signature, rec_id) = app
            .sign_message(signing_input, account_index)
            .expect("Msg signature");

        let msg_length = VarInt(signing_input.len() as u64).consensus_encode();

        let btc_message = {
            let mut temp = Vec::from("\x18Bitcoin Signed Message:\n");
            temp.extend(&msg_length);
            temp.extend(signing_input);
            temp
        };

        let hash = Sha256::new_with_prefix(btc_message).finalize();
        let digest = Sha256::new_with_prefix(hash);

        let recov_key =
            VerifyingKey::recover_from_digest(digest.clone(), &signature, rec_id).unwrap();

        assert_eq!(recov_key, verif_key);

        verif_key
            .verify_digest(digest, &signature)
            .expect("Verification");
    }
}

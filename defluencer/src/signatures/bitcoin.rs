#![cfg(not(target_arch = "wasm32"))]

use async_trait::async_trait;

use bitcoin::{consensus::Encodable, VarInt};

use sha2::{Digest, Sha256};

use crate::errors::Error;

use super::{ledger::BitcoinLedgerApp, signed_link::HashAlgorithm};

#[derive(Clone)]
pub struct BitcoinSigner {
    app: BitcoinLedgerApp,
    account_index: u32,
}

impl BitcoinSigner {
    pub fn new(app: BitcoinLedgerApp, account_index: u32) -> Self {
        Self { app, account_index }
    }
}

#[async_trait(?Send)]
impl super::Signer for BitcoinSigner {
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
        let signature = self.app.sign_message(signing_input, self.account_index)?;

        let btc_message = {
            let mut temp = Vec::from("\x18Bitcoin Signed Message:\n");

            let mut msg_len = Vec::with_capacity(9); // Bicoin style Varint
            VarInt(signing_input.len() as u64)
                .consensus_encode(&mut msg_len)
                .expect("VarInt encoded message length");

            temp.extend(&msg_len);
            temp.extend(signing_input);
            temp
        };

        let hash = Sha256::new_with_prefix(btc_message).finalize();
        let digest = Sha256::new_with_prefix(hash);
        let recovered_key = signature.recover_verifying_key_from_digest(digest)?;

        let signature = k256::ecdsa::Signature::from(signature);

        Ok((recovered_key, signature, HashAlgorithm::BitcoinLedgerApp))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use bitcoin::VarInt;

    use k256::ecdsa::VerifyingKey;

    use sha2::Digest;

    use signature::DigestVerifier;

    #[test]
    fn addr() {
        let app = BitcoinLedgerApp::default();

        let (_, addr) = app.get_extended_pubkey(0).unwrap();

        println!("Address: {}", addr);
    }

    #[test]
    fn sign_test() {
        let app = BitcoinLedgerApp::default();
        let account_index = 0;

        let (pub_key, _addr) = app.get_extended_pubkey(account_index).unwrap();
        let verif_key = VerifyingKey::from(pub_key);

        //let signing_input = b"Hello World!";
        let signing_input = b"The root problem with conventional currency is all the trust that's required to make it work. The central bank must be trusted not to debase the currency, but the history of fiat currencies is full of breaches of that trust. Banks must be trusted to hold our money and transfer it electronically, but they lend it out in waves of credit bubbles with barely a fraction in reserve. We have to trust them with our privacy, trust them not to let identity thieves drain our accounts. Their massive overhead costs make micropayments impossible.";

        let display_hash = Sha256::new_with_prefix(signing_input).finalize();
        println!("Message Display Hash: 0x{}", hex::encode(display_hash));

        let signature = app
            .sign_message(signing_input, account_index)
            .expect("Msg signature");

        let msg_length = {
            let mut temp = Vec::with_capacity(9); // Bicoin style Varint
            VarInt(signing_input.len() as u64)
                .consensus_encode(&mut temp)
                .expect("VarInt encoded message length");
            temp
        };

        let btc_message = {
            let mut temp = Vec::from("\x18Bitcoin Signed Message:\n");
            temp.extend(&msg_length);
            temp.extend(signing_input);
            temp
        };

        let hash = Sha256::new_with_prefix(btc_message).finalize();
        let digest = Sha256::new_with_prefix(hash);

        let recov_key = signature
            .recover_verifying_key_from_digest(digest.clone())
            .unwrap();

        assert_eq!(recov_key, verif_key);

        verif_key
            .verify_digest(digest, &signature)
            .expect("Verification");
    }
}

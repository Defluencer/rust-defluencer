#![cfg(not(target_arch = "wasm32"))]

use async_trait::async_trait;

use bitcoin::{consensus::Encodable, VarInt};

use sha2::{Digest, Sha256};

use signature::DigestVerifier;

use crate::errors::Error;

use super::ledger::BitcoinLedgerApp;

#[derive(Clone)]
pub struct BitcoinSigner {
    app: BitcoinLedgerApp,
}

impl BitcoinSigner {
    pub fn new() -> Self {
        let app = BitcoinLedgerApp::default();

        Self { app }
    }
}

#[async_trait(?Send)]
impl super::Signer for BitcoinSigner {
    async fn sign(
        &self,
        signing_input: Vec<u8>,
    ) -> Result<(k256::PublicKey, k256::ecdsa::Signature), Error> {
        let btc_message = {
            let mut temp = Vec::from("\x18Bitcoin Signed Message:\n");

            let mut msg_len = Vec::with_capacity(9); // Bicoin style Varint
            VarInt(signing_input.len() as u64)
                .consensus_encode(&mut msg_len)
                .expect("VarInt encoded message length");

            temp.extend(&msg_len);
            temp.extend(signing_input.iter());
            temp
        };

        let signature = self.app.sign_message(&signing_input, 0)?;

        let digest = Sha256::new_with_prefix(btc_message);
        let recovered_key = signature.recover_verifying_key_from_digest(digest.clone())?;
        recovered_key.verify_digest(digest, &signature)?;

        let public_key = k256::PublicKey::from(recovered_key);
        let signature = k256::ecdsa::Signature::from(signature);

        Ok((public_key, signature))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use bitcoin::VarInt;

    use sha2::Digest;
    use signature::DigestVerifier;

    #[test]
    fn addr() {
        let app = BitcoinLedgerApp::default();

        let addr = app.get_extended_pubkey(0).unwrap();

        println!("Address: {}", addr);
    }

    #[test]
    fn sign() {
        let app = BitcoinLedgerApp::default();

        let signing_input = b"Hello World!";
        //let signing_input = b"The root problem with conventional currency is all the trust that's required to make it work. The central bank must be trusted not to debase the currency, but the history of fiat currencies is full of breaches of that trust. Banks must be trusted to hold our money and transfer it electronically, but they lend it out in waves of credit bubbles with barely a fraction in reserve. We have to trust them with our privacy, trust them not to let identity thieves drain our accounts. Their massive overhead costs make micropayments impossible.";

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

        let mut hasher = sha2::Sha256::new();

        hasher.update(&signing_input);
        let display_hash = hasher.finalize();

        println!("Message Display Hash: 0x{}", hex::encode(display_hash));

        let signature = app.sign_message(signing_input, 0).expect("Msg signature");

        let digest = Sha256::new_with_prefix(btc_message);

        let recovered_key = signature
            .recover_verifying_key_from_digest(digest.clone())
            .expect("Key recovery");

        recovered_key
            .verify_digest(digest, &signature)
            .expect("Verification");
    }
}

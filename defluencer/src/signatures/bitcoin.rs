#![cfg(not(target_arch = "wasm32"))]

use async_trait::async_trait;

use bitcoin::{consensus::Encodable, VarInt};

use sha2::{Digest, Sha256};

use signature::Verifier;

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

        let mut hasher = Sha256::new();
        hasher.update(btc_message.clone());
        let hash = hasher.finalize_reset();

        hasher.update(hash);
        let hash = hasher.finalize();

        let signature = self.app.sign_message(&signing_input, 0)?;

        let recovered_key = signature.recover_verifying_key_from_digest_bytes(&hash)?;

        recovered_key.verify(&btc_message, &signature)?;

        let public_key = k256::PublicKey::from(recovered_key);
        let signature = k256::ecdsa::Signature::from(signature);

        Ok((public_key, signature))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use bitcoin::VarInt;

    use sha2::{Digest, Sha256};

    #[test]
    fn addr() {
        let app = BitcoinLedgerApp::default();

        let addr = app.get_extended_pubkey(0).unwrap();

        println!("Address: {}", addr);
    }

    #[test]
    fn sign() {
        use signature::Verifier;

        let app = BitcoinLedgerApp::default();

        //let signing_input = b"Hello World!";
        let signing_input = b"The root problem with conventional currency is all the trust that's required to make it work. The central bank must be trusted not to debase the currency, but the history of fiat currencies is full of breaches of that trust. Banks must be trusted to hold our money and transfer it electronically, but they lend it out in waves of credit bubbles with barely a fraction in reserve. We have to trust them with our privacy, trust them not to let identity thieves drain our accounts. Their massive overhead costs make micropayments impossible.";

        /* let merkle_root = vec![
            127, 131, 177, 101, 127, 241, 252, 83, 185, 45, 193, 129, 72, 161, 214, 93, 252, 45,
            75, 31, 163, 214, 119, 40, 74, 221, 210, 0, 18, 109, 144, 105,
        ]; */

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

        let mut hasher = Sha256::new();
        hasher.update(btc_message.clone());
        let hash = hasher.finalize_reset();

        hasher.update(hash);
        let hash = hasher.finalize();

        println!("Message Display Hash: 0x{}", hex::encode(hash));

        let signature = app.sign_message(signing_input, 0).unwrap();

        let recovered_key = signature
            .recover_verifying_key_from_digest_bytes(&hash)
            .unwrap();

        recovered_key.verify(&hash, &signature).unwrap();
    }
}

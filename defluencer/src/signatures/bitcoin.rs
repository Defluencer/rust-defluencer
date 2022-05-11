use async_trait::async_trait;

use crate::errors::Error;

//use signature::Signature;

// https://github.com/LedgerHQ/app-bitcoin-new/blob/develop/doc/bitcoin.md#sign_message
// https://docs.rs/bitcoin/0.28.1/bitcoin/consensus/encode/struct.VarInt.html
// https://docs.rs/bitcoin/0.28.1/bitcoin/util/misc/fn.signed_msg_hash.html
// https://docs.rs/bitcoin/0.28.1/bitcoin/util/hash/fn.bitcoin_merkle_root.html

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
pub struct BitcoinSigner {}

#[cfg(not(target_arch = "wasm32"))]
impl BitcoinSigner {
    pub fn new() -> Self {
        Self {}
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait(?Send)]
impl super::Signer for BitcoinSigner {
    async fn sign(
        &self,
        _signing_input: Vec<u8>,
    ) -> Result<(k256::PublicKey, k256::ecdsa::Signature), Error> {
        unimplemented!();

        /* let msg_length = signing_input.len(); //TODO encode as bitcoin varint https://wiki.bitcoinsv.io/index.php/VarInt

        let mut eth_message = format!("\x18Bitcoin Signed Message:\n{}", msg_length).into_bytes();
        eth_message.extend_from_slice(&signing_input);

        //TODO sign

        // TODO Bitcoin ledger app return Id as the first byte. Put it at the end so it conform to k256 crate.
        let signature = k256::ecdsa::recoverable::Signature::from_bytes(&sig.to_fixed_bytes())?;

        let recovered_key = signature.recover_verifying_key(&eth_message)?; // The fn hash the message

        let public_key = k256::PublicKey::from(recovered_key);
        let signature = k256::ecdsa::Signature::from(signature);

        Ok((public_key, signature)) */
    }
}

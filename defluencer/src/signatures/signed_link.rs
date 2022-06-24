use linked_data::types::IPLDLink;

use serde::{Deserialize, Serialize};

use signature::DigestVerifier;

/// Verification is done by applying the hash algo to the CID's hash then verifiying with ECDSA.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SignedLink {
    /// The root hash of the DAG being signed.
    pub link: IPLDLink,

    /// SEC1 encoded public key with point compression applied.
    pub public_key: Vec<u8>,

    /// What algo to apply before signing
    pub hash_algo: HashAlgorithm,

    /// ASN.1 DER encoded signature.
    pub signature: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum HashAlgorithm {
    BitcoinLedgerApp,
    EthereumLedgerApp,
}

impl SignedLink {
    pub fn verify(&self) -> bool {
        match self.hash_algo {
            HashAlgorithm::BitcoinLedgerApp => self.verify_btc(),
            HashAlgorithm::EthereumLedgerApp => self.verify_eth(),
        }
    }

    fn verify_btc(&self) -> bool {
        use bitcoin::{consensus::Encodable, VarInt};
        use sha2::{Digest, Sha256};

        let signing_input = self.link.link.hash().digest();

        let verif_key = match k256::ecdsa::VerifyingKey::from_sec1_bytes(&self.public_key) {
            Ok(key) => key,
            Err(_) => return false,
        };

        let signature = match k256::ecdsa::Signature::from_der(&self.signature) {
            Ok(sig) => sig,
            Err(_) => return false,
        };

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

        verif_key.verify_digest(digest, &signature).is_ok()
    }

    fn verify_eth(&self) -> bool {
        use sha3::{Digest, Keccak256};

        let signing_input = self.link.link.hash().digest();

        let verif_key = match k256::ecdsa::VerifyingKey::from_sec1_bytes(&self.public_key) {
            Ok(key) => key,
            Err(_) => return false,
        };

        let signature = match k256::ecdsa::Signature::from_der(&self.signature) {
            Ok(sig) => sig,
            Err(_) => return false,
        };

        let mut eth_message =
            format!("\x19Ethereum Signed Message:\n{}", signing_input.len()).into_bytes();
        eth_message.extend_from_slice(signing_input);

        let digest = Keccak256::new_with_prefix(eth_message);

        verif_key.verify_digest(digest, &signature).is_ok()
    }
}

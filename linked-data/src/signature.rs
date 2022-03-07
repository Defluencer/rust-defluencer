use crate::{keccak256, Address};

use serde::{Deserialize, Serialize};

use serde_with::serde_as;

use libsecp256k1::{recover, Message, RecoveryId, Signature};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// TODO DAG-JOSE; build a specific type of JWS for ethereum keys.
// https://ipld.io/specs/codecs/dag-jose/fixtures/
// https://ipld.io/specs/codecs/dag-jose/spec/
// https://www.rfc-editor.org/rfc/rfc7515
// https://www.rfc-editor.org/rfc/rfc7517
// https://www.rfc-editor.org/rfc/rfc7518

/// Generic crypto-signed message.
#[serde_as]
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct SignedMessage<T>
where
    T: Serialize,
{
    pub address: Address,

    pub data: T,

    #[serde_as(as = "[_; 65]")]
    pub signature: [u8; 65],
}

impl<T> SignedMessage<T>
where
    T: Serialize,
{
    pub fn verify(&self) -> bool {
        let public_key = match self.public_key() {
            Ok(key) => key,
            Err(_) => return false,
        };

        // The public key returned is 65 bytes long, that is because it is prefixed by `0x04` to indicate an uncompressed public key.
        let hash = keccak256(&public_key[1..]);

        // The public address is defined as the low 20 bytes of the keccak hash of the public key.
        hash[12..] == self.address
    }

    fn public_key(&self) -> Result<[u8; 65]> {
        let message = serde_json::to_vec(&self.data)?;

        let mut eth_message =
            format!("\x19Ethereum Signed Message:\n{}", message.len()).into_bytes();
        eth_message.extend_from_slice(&message);

        let hash = keccak256(&eth_message);

        let msg = Message::parse_slice(&hash)?;

        let sig = Signature::parse_standard_slice(&self.signature[0..64])?;

        let rec_id = match RecoveryId::parse_rpc(self.signature[64]) {
            Ok(id) => id,
            Err(_) => RecoveryId::parse(self.signature[64])?,
        };

        let data = recover(&msg, &sig, &rec_id)?;

        Ok(data.serialize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::chat::ChatId;
    use crate::peer_id_from_str;

    /// Real test done with my ledger nano S.
    #[test]
    fn ledger_test() {
        let address = [
            104, 120, 68, 17, 76, 204, 129, 156, 66, 98, 202, 140, 162, 28, 39, 230, 46, 68, 232,
            120,
        ];

        let peer_id =
            peer_id_from_str("12D3KooWRsEKtLGLW9FHw7t7dDhHrMDahw3VwssNgh55vksdvfmC").unwrap();

        let data = ChatId {
            name: "sionois.eth".to_owned(),
            peer_id,
        };

        let signed_msg = SignedMessage::<ChatId> {
            address,
            data,
            signature: [
                100, 68, 201, 51, 155, 12, 98, 187, 235, 200, 154, 126, 50, 194, 231, 102, 128,
                130, 182, 21, 10, 132, 63, 225, 219, 62, 125, 123, 173, 186, 73, 104, 22, 79, 209,
                48, 72, 222, 118, 109, 165, 130, 244, 193, 85, 1, 89, 205, 229, 234, 160, 89, 204,
                157, 108, 21, 44, 218, 200, 47, 19, 112, 28, 213, 0,
            ],
        };

        assert!(signed_msg.verify());
    }

    #[test]
    fn metamask_test() {
        let address = [
            144, 182, 177, 234, 11, 229, 143, 176, 142, 170, 181, 114, 142, 69, 78, 70, 56, 185,
            41, 242,
        ];

        let peer_id =
            peer_id_from_str("12D3KooWRsEKtLGLW9FHw7t7dDhHrMDahw3VwssNgh55vksdvfmC").unwrap();

        let data = ChatId {
            name: "SionoiS".to_owned(),
            peer_id,
        };

        let signed_msg = SignedMessage::<ChatId> {
            address,
            data,
            signature: [
                25, 56, 168, 88, 243, 119, 179, 52, 151, 139, 10, 171, 188, 36, 73, 138, 220, 79,
                104, 49, 69, 104, 133, 230, 253, 129, 235, 110, 188, 213, 241, 13, 107, 149, 155,
                188, 235, 220, 154, 56, 169, 59, 30, 112, 72, 67, 194, 11, 13, 18, 158, 32, 84,
                198, 14, 216, 34, 61, 152, 226, 88, 226, 49, 175, 28,
            ],
        };

        assert!(signed_msg.verify());
    }
}

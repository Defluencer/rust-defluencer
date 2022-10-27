use crate::channel::moderation::{Ban, Moderator};

use crate::types::{IPLDLink, PeerId};

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct ChatMessage {
    /// Usualy text, ban user or add moderator.
    pub message: MessageType,

    /// Link to DAG-JOSE block for verification.
    pub signature: IPLDLink,
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
#[serde(untagged)]
pub enum MessageType {
    Text(String),
    Ban(Ban),
    Mod(Moderator),
}

/// The purpose of signing this data is to mitigate identity theft.
///
/// Since chat sessions have definite start times, the latest block hash can be used,
/// in conjuction with a signature to achive adequate security without requiring the user
/// to sign every message.
///
/// This scheme make local IPFS node keys theft less of a bulletproof way to impersonate someone.
/// Rotating Peer Id and signing again would end the attack and the attacker would have to wait for
/// the real user to start chatting before attacking, making it very obvious.
///
/// Every chat implementation would have to invalidate the old chat ID when the same public key sign a new chat ID
///
/// W.I.P.
#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct ChatInfo {
    /// Name of the sender
    pub name: String,

    /// Node used to chat
    pub node: PeerId,
    // Latest Block Hash
    //pub latest_block_hash: Vec<u8>,
}

use crate::moderation::{Ban, Moderator};

use crate::types::{IPLDLink, PeerId};

use serde::{Deserialize, Serialize};

/// CID of crypto-signed ChatID.
pub type ChatSig = cid::Cid;

/// GossipSub Live Chat Message.
#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct ChatMessage {
    /// Usualy text, ban user or add moderator.
    pub message: MessageType,

    /// Link to chat ID, crypto-signed.
    pub signature: IPLDLink,
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
#[serde(untagged)]
pub enum MessageType {
    Text(String),
    Ban(Ban),
    Mod(Moderator),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatId {
    pub peer_id: PeerId,

    pub name: String,
}

use crate::{
    moderation::{Ban, Moderator},
    IPLDLink, PeerId,
};

use serde::{Deserialize, Serialize};

use serde_with::{serde_as, DisplayFromStr};

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

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatId {
    #[serde_as(as = "DisplayFromStr")]
    pub peer_id: PeerId,

    pub name: String,
}

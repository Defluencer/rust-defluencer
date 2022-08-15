use crate::moderation::{Ban, Moderator};

use crate::types::{IPLDLink, PeerId};

use serde::{Deserialize, Serialize};

/// GossipSub Live Chat Message.
#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct ChatMessage {
    /// Usualy text, ban user or add moderator.
    pub message: MessageType,

    /// Link to DAG-JOSE block for verification.
    /// This block links to ChatInfo
    pub signature: IPLDLink,
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
#[serde(untagged)]
pub enum MessageType {
    Text(String),
    Ban(Ban),
    Mod(Moderator),
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct ChatInfo {
    /// Name of the sender
    pub name: String,

    /// Node used to chat
    pub node: PeerId,
}

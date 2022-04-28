use crate::moderation::{Ban, Moderator};

use crate::types::IPLDLink;

use serde::{Deserialize, Serialize};

/// GossipSub Live Chat Message.
#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct ChatMessage {
    /// Name of the sender
    pub name: String,

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

use crate::types::{IPLDLink, PeerId};

use serde::{Deserialize, Serialize};

/// Chat & Video streaming settings
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct LiveSettings {
    /// Peer Id of the streaming node
    pub peer_id: PeerId,

    /// PubSub topic for the live streaming.
    pub video_topic: String,

    /// Should stream be archived.
    pub archiving: bool,

    /// PubSub topic for the live chat.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_topic: Option<String>,

    /// Link to banned users address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bans: Option<IPLDLink>,

    /// Link to moderators address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mods: Option<IPLDLink>,
}

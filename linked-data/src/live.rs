use crate::types::PeerId;

use serde::{Deserialize, Serialize};

/// Stream settings

#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct LiveSettings {
    /// Peer Id of the streaming node
    pub peer_id: PeerId,

    /// PubSub topic for the live streaming.
    pub video_topic: String,

    /// PubSub topic form the live chat.
    pub chat_topic: String,
}

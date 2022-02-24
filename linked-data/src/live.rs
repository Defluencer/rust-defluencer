use crate::PeerId;

use serde::{Deserialize, Serialize};

use serde_with::{serde_as, DisplayFromStr};

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct Live {
    #[serde_as(as = "DisplayFromStr")]
    pub peer_id: PeerId,

    /// PubSub topic for the live streaming.
    pub video_topic: String,

    /// PubSub topic form the live chat.
    pub chat_topic: String,
}

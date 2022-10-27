pub mod follows;
pub mod live;
pub mod moderation;

use crate::types::IPLDLink;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone, Default)]
pub struct ChannelMetadata {
    pub identity: IPLDLink,

    /// Link to chronological tree of all a channel's content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_index: Option<IPLDLink>,

    /// Link to HAMT containing all the channel comments.
    ///
    /// Keys = Content CIDs
    ///
    /// Value = HAMT containing comments
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment_index: Option<IPLDLink>,

    /// Link to live stream settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live: Option<IPLDLink>,

    /// Link to list of followees.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub follows: Option<IPLDLink>,

    /// Pubsub channel topic for aggregation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agregation_channel: Option<String>,
}

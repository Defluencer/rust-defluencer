use crate::types::IPLDLink;

use serde::{Deserialize, Serialize};

/// Non exhaustive list of links to various social media features.
///
/// The Cid of this object should be publicly available and trusted to be the latest version.
#[derive(Deserialize, Serialize, PartialEq, Debug, Clone, Default)]
pub struct ChannelMetadata {
    pub identity: IPLDLink,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_index: Option<IPLDLink>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment_index: Option<IPLDLink>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub live: Option<IPLDLink>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub follows: Option<IPLDLink>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub agregation_channel: Option<String>,
}

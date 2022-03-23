use crate::{indexes::Indexing, IPLDLink};

use serde::{Deserialize, Serialize};

/// Non exhaustive list of links to various social media features.
///
/// The Cid of this object should be publicly available and trusted to be the latest version.
/// Blockchains are best suited for this.
#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq)]
pub struct ChannelMetadata {
    pub identity: IPLDLink,

    pub content_index: Indexing,

    pub comment_index: Indexing,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub live: Option<IPLDLink>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub follows: Option<IPLDLink>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub bans: Option<IPLDLink>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub mods: Option<IPLDLink>,
}

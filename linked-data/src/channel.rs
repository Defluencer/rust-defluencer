use crate::{follows::Follows, identity::Identity, live::LiveSettings, IPLDLink};

use serde::{Deserialize, Serialize};

/// Non exhaustive list of links to various social media features.
///
/// The Cid of this object should be publicly available and trusted to be the latest version.
/// Blockchains are best suited for this.
#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq)]
pub struct ChannelMetadata {
    pub identity: Option<Identity>,
    pub content_index: Option<Indexing>,
    pub comment_index: Option<Indexing>,
    pub live: Option<LiveSettings>,
    pub follows: Option<Follows>,
    pub bans: Option<IPLDLink>,
    pub mods: Option<IPLDLink>,
}

#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq)]
pub struct Indexing {
    pub date_time: IPLDLink,
}

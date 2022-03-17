use crate::{
    comments::CommentIndexing, content::ContentIndexing, follows::Follows, identity::Identity,
    live::LiveSettings, IPLDLink,
};

use serde::{Deserialize, Serialize};

/// Non exhaustive list of links to various social media features.
///
/// The Cid of this object should be publicly available and trusted to be the latest version.
/// Blockchains are best suited for this.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct ChannelMetadata {
    pub identity: Identity,
    pub content_index: ContentIndexing,
    pub comment_index: CommentIndexing,
    pub live: Option<LiveSettings>,
    pub follows: Option<Follows>,
    pub bans: Option<IPLDLink>,
    pub mods: Option<IPLDLink>,
}

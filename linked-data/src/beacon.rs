use crate::{
    comments::CommentIndexing, content::ContentIndexing, follows::Follows, identity::Identity,
    live::LiveSettings, IPLDLink,
};

use serde::{Deserialize, Serialize};

/// Non exhaustive list of links to various social media features.
///
/// The Cid of this object should be publicly available and trusted to be up to date.
/// Blockchains are best suited for this.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct Beacon {
    pub identity: Identity,
    pub content: ContentIndexing,
    pub comments: CommentIndexing,
    pub live: Option<LiveSettings>,
    pub follows: Option<Follows>,
    pub bans: Option<IPLDLink>,
    pub mods: Option<IPLDLink>,
}

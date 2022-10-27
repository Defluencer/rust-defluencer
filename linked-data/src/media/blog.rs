use crate::types::IPLDLink;

use serde::{Deserialize, Serialize};

/// Metadata for a blog post, article or essay.
#[derive(Deserialize, Serialize, PartialEq, Clone, Debug, Default)]
pub struct BlogPost {
    /// Creator identity link
    pub identity: IPLDLink,

    /// Timestamp at the time of publication in Unix time
    pub user_timestamp: i64,

    /// Link to markdown file
    pub content: IPLDLink,

    /// The title of this blog post
    pub title: String,

    /// Link to thumbnail image
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<IPLDLink>,

    /// Number of words in the text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word_count: Option<u64>,
}

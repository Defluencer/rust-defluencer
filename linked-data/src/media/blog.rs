use crate::IPLDLink;

use serde::{Deserialize, Serialize};

/// A micro blog post (Twitter-sytle).
///
/// Recursive pin.
#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
pub struct MicroPost {
    pub identity: IPLDLink,

    /// Timestamp at the time of publication in Unix time.
    pub user_timestamp: i64,

    /// Text as content of the blog post.
    pub content: String,
}

/// Metadata for a long blog post.
///
/// Recursive pin.
#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
pub struct FullPost {
    pub identity: IPLDLink,

    /// Timestamp at the time of publication in Unix time.
    pub user_timestamp: i64,

    /// Link to markdown file
    pub content: IPLDLink,

    /// Link to thumbnail image.
    pub image: IPLDLink,

    /// The title of this blog post
    pub title: String,
}

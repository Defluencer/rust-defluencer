use crate::types::IPLDLink;

use cid::Cid;

use serde::{Deserialize, Serialize};

use serde_with::{serde_as, DisplayFromStr};

/// Comment metadata and text.
#[serde_as]
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Default)]
pub struct Comment {
    pub identity: IPLDLink,

    /// Timestamp at the time of publication in Unix time.
    pub user_timestamp: i64,

    /// Link to the content being commented on.
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<Cid>,

    /// Text as content of the comment.
    pub text: String,
}

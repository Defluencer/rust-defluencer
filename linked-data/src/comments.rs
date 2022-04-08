use crate::IPLDLink;

use cid::Cid;

use serde::{Deserialize, Serialize};

use serde_with::{serde_as, DisplayFromStr};

/// Comment metadata and text.
#[serde_as]
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Comment {
    pub identity: IPLDLink,

    /// Timestamp at the time of publication in Unix time.
    pub user_timestamp: i64,

    /// Link to the content being commented on.
    #[serde_as(as = "DisplayFromStr")]
    pub origin: Cid,

    /// Text as content of the comment.
    pub text: String,
}

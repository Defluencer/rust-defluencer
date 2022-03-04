use crate::IPLDLink;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use cid::Cid;
use serde_with::{serde_as, DisplayFromStr};

#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq)]
pub struct CommentIndexing {
    pub date_time: Option<IPLDLink>,
}

//TODO Implement a HAMT. It would scale to any size unlike the current solution.

#[serde_as]
#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq)]
pub struct Comments {
    /// Content cids mapped to comments.
    #[serde_as(as = "HashMap<DisplayFromStr, Vec<_>>")]
    pub comments: HashMap<Cid, Vec<IPLDLink>>,
}

/// Comment metadata and text.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Comment {
    /// Timestamp at the time of publication in Unix time.
    pub timestamp: i64,

    /// Link to the content being commented on.
    pub origin: IPLDLink,

    /// Text as content of the comment.
    pub text: String,
}

/* #[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn serde_test() {
        let mut old_comments = Commentary {
            comments: HashMap::with_capacity(2),
        };

        let cid =
            Cid::from_str("bafyreibjo4xmgaevkgud7mbifn3dzp4v4lyaui4yvqp3f2bqwtxcjrdqg4").unwrap();

        old_comments
            .comments
            .insert(cid, vec![Cid::default().into()]);
        old_comments
            .comments
            .insert(cid, vec![Cid::default().into()]);

        let json = serde_json::to_string_pretty(&old_comments).expect("Cannot Serialize");
        println!("{}", json);

        let new_comments = serde_json::from_str(&json).expect("Cannot Deserialize");
        println!("{:?}", new_comments);

        assert_eq!(old_comments, new_comments);
    }
} */

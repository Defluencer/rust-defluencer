use serde::{Deserialize, Serialize};

use crate::IPLDLink;

#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq)]
pub struct ChainLink {
    /// Link to any media; video, blog, comment, etc...
    pub media: IPLDLink,

    /// Link to the previous media to form a chain.
    pub previous: Option<IPLDLink>,
}

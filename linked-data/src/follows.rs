use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::types::IPLDLink;

/// List of followed users.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct Follows {
    /// Links to identity of followed channels or users.
    pub followees: HashSet<IPLDLink>,
}

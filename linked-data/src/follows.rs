use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::types::IPNSAddress;

/// List of followed users.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct Follows {
    /// Addresses of followed channels.
    pub followees: HashSet<IPNSAddress>,
}

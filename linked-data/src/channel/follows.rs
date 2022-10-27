use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::types::IPNSAddress;

/// List of followees.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct Follows {
    pub followees: HashSet<IPNSAddress>,
}

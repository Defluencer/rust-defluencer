use std::collections::HashSet;

use cid::Cid;

use serde::{Deserialize, Serialize};

use serde_with::{serde_as, DisplayFromStr};

/// List of followed users.
#[serde_as]
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Eq, Clone)]
pub struct Follows {
    pub ens: HashSet<String>,

    #[serde_as(as = "HashSet<DisplayFromStr>")]
    pub ipns: HashSet<Cid>,
}

impl Follows {
    pub fn is_empty(&self) -> bool {
        self.ens.is_empty() || self.ipns.is_empty()
    }
}

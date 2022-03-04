use std::collections::HashSet;

use cid::Cid;

use serde::{Deserialize, Serialize};

use serde_with::{serde_as, DisplayFromStr};

/// List of who you follow.
#[serde_as]
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Eq, Clone)]
pub struct Follows {
    pub ens: HashSet<String>,

    #[serde_as(as = "HashSet<DisplayFromStr>")]
    pub ipns: HashSet<Cid>,
}

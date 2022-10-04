use std::collections::{BTreeMap, HashSet};

use crate::types::IPLDLink;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
pub struct Yearly {
    pub year: BTreeMap<i32, IPLDLink>,
}

#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
pub struct Monthly {
    pub month: BTreeMap<u32, IPLDLink>,
}

#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
pub struct Daily {
    pub day: BTreeMap<u32, IPLDLink>,
}

#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
pub struct Hourly {
    pub hour: BTreeMap<u32, IPLDLink>,
}

#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
pub struct Minutes {
    pub minute: BTreeMap<u32, IPLDLink>,
}

#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
pub struct Seconds {
    pub second: BTreeMap<u32, HashSet<IPLDLink>>,
}

use std::collections::BTreeMap;

use crate::IPLDLink;

use serde::{Deserialize, Serialize};

//TODO Implement a HAMT.

#[derive(Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct Yearly {
    pub year: BTreeMap<i32, IPLDLink>,
}

#[derive(Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct Monthly {
    pub month: BTreeMap<u32, IPLDLink>,
}

#[derive(Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct Daily {
    pub day: BTreeMap<u32, IPLDLink>,
}

#[derive(Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct Hourly {
    pub hour: BTreeMap<u32, IPLDLink>,
}

#[derive(Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct Minutes {
    pub minute: BTreeMap<u32, IPLDLink>,
}

#[derive(Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct Seconds {
    pub second: BTreeMap<u32, IPLDLink>,
}

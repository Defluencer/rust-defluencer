use std::collections::HashMap;

use crate::IPLDLink;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct Yearly {
    pub year: HashMap<i32, IPLDLink>,
}

#[derive(Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct Monthly {
    pub month: HashMap<u32, IPLDLink>,
}

#[derive(Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct Daily {
    pub day: HashMap<u32, IPLDLink>,
}

#[derive(Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct Hourly {
    pub hour: HashMap<u32, IPLDLink>,
}

#[derive(Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct Minutes {
    pub minute: HashMap<u32, IPLDLink>,
}

#[derive(Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct Seconds {
    pub second: HashMap<u32, IPLDLink>,
}

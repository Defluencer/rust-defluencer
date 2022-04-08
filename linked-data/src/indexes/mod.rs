pub mod date_time;
pub mod hamt;
pub mod log;

use crate::IPLDLink;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq)]
pub struct Indexing {
    pub log: Option<IPLDLink>,
    pub date_time: Option<IPLDLink>,
    pub hamt: Option<IPLDLink>,
}

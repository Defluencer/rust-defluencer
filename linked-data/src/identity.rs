use crate::IPLDLink;

use cid::Cid;

use serde::{Deserialize, Serialize};

use serde_with::{serde_as, DisplayFromStr};

#[serde_as]
#[derive(Serialize, Deserialize, PartialEq, Default, Clone, Debug)]
pub struct Identity {
    pub display_name: String,

    pub avatar: IPLDLink,

    /// IPNS address.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub channel_ipns: Option<Cid>,

    /// Ethereum Name Service domain name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_ens: Option<String>,
}

//TODO impl default with generic avatar

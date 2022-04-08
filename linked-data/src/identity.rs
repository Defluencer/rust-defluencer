use crate::{IPLDLink, IPNSAddress};

use serde::{Deserialize, Serialize};

use serde_with::{serde_as, DisplayFromStr};

#[serde_as]
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct Identity {
    /// Public choosen name.
    pub display_name: String,

    /// Mime-typed image link.
    pub avatar: IPLDLink,

    /// IPNS address.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub channel_ipns: Option<IPNSAddress>,

    /// ENS domain name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_ens: Option<String>,
}

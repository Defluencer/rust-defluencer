use crate::types::{IPLDLink, IPNSAddress};

use serde::{Deserialize, Serialize};

/// Minimum viable social media identity.
///
/// A public key hash is all that is needed.
///
/// Current system use multiple keys but one key in a HW could sign IPNS records AND DAG-JOSE blocks.
/// Just need to build the app for that.
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct Identity {
    /// Public choosen name.
    pub display_name: String,

    /// Mime-typed image link.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<IPLDLink>,

    /// IPNS address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_ipns: Option<IPNSAddress>,

    /// Blockchain address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub addr: Option<String>,
}

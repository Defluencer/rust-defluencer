use crate::types::{IPLDLink, IPNSAddress};

use serde::{Deserialize, Serialize};

/// Minimum viable social media identity.
///
/// A public key hash is all that is needed.
///
/// Current system use multiple keys but one key in a HW could sign IPNS records AND DAG-JOSE blocks.
/// Just need to build the app for that.
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub struct Identity {
    /// Choosen name.
    pub name: String,

    /// User short biography.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bio: Option<String>,

    /// Link to background image
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banner: Option<IPLDLink>,

    /// Avatar image link.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<IPLDLink>,

    /// IPNS address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipns_addr: Option<IPNSAddress>,

    /// Bitcoin address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub btc_addr: Option<String>,

    /// Ethereum address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eth_addr: Option<String>,
}

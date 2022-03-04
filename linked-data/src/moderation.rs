use crate::{Address, PeerId};

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use serde_with::{serde_as, DisplayFromStr};

/// List of banned users.
/// Direct pin.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
pub struct Bans {
    pub banned_addrs: HashSet<Address>,
}

/// List of moderators.
/// Direct pin.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
pub struct Moderators {
    pub moderator_addrs: HashSet<Address>,
}

/// Message to ban/unban a user.
/// Should be signed by a moderator.
#[serde_as]
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Ban {
    #[serde_as(as = "DisplayFromStr")]
    pub ban_peer: PeerId,

    pub ban_addrs: Address,
}

/// Message to mod/unmod a user.
/// Should be signed by an administrator.
#[serde_as]
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Moderator {
    #[serde_as(as = "DisplayFromStr")]
    pub mod_peer: PeerId,

    pub mod_addrs: Address,
}

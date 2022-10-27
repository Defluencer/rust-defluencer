use crate::types::{Address, PeerId};

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

//TODO use HAMT to store identities.

/// List of banned users.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct Bans {
    pub banned_addrs: HashSet<Address>,
}

/// List of moderators.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct Moderators {
    pub moderator_addrs: HashSet<Address>,
}

/// Message to ban/unban a user.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Ban {
    pub ban_peer: PeerId,

    pub ban_addrs: Address,
}

/// Message to mod/unmod a user.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Moderator {
    pub mod_peer: PeerId,

    pub mod_addrs: Address,
}

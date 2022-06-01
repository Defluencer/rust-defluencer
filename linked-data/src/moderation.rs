use crate::types::{Address, PeerId};

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// List of banned users.
/// Direct pin.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct Bans {
    pub banned_addrs: HashSet<Address>, // Could also use HAMT to store crypto address since they are hash based.
}

/// List of moderators.
/// Direct pin.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct Moderators {
    pub moderator_addrs: HashSet<Address>, // Could also use HAMT to store crypto address since they are hash based.
}

/// Message to ban/unban a user.
/// Should be signed by a moderator.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Ban {
    pub ban_peer: PeerId,

    pub ban_addrs: Address,
}

/// Message to mod/unmod a user.
/// Should be signed by an administrator.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Moderator {
    pub mod_peer: PeerId,

    pub mod_addrs: Address,
}

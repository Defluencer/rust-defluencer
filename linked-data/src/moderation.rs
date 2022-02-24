use crate::{chat::ChatSig, Address, PeerId};

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use serde_with::{serde_as, DisplayFromStr};

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

/// Local cache of who is verified and/or banned.
pub struct ChatModerationCache {
    verified: HashMap<PeerId, usize>, // Map peer IDs to indices.

    peers: Vec<PeerId>,      // sync
    origins: Vec<ChatSig>,   // sync
    addresses: Vec<Address>, // sync
    names: Vec<String>,      // sync not used when archiving

    ban_index: usize, // Lower than this users are banned.
}

impl ChatModerationCache {
    pub fn new(capacity: usize, name_cap: usize) -> Self {
        Self {
            verified: HashMap::with_capacity(capacity),

            peers: Vec::with_capacity(capacity),
            origins: Vec::with_capacity(capacity),
            addresses: Vec::with_capacity(capacity),
            names: Vec::with_capacity(name_cap),

            ban_index: 0,
        }
    }

    /// Check if this peer is banned.
    pub fn is_banned(&self, peer: &PeerId) -> bool {
        let index = match self.verified.get(peer) {
            Some(i) => *i,
            None => return false,
        };

        index < self.ban_index
    }

    /// Check if this peer is verified.
    pub fn is_verified(&self, peer: &PeerId, origin: &ChatSig) -> bool {
        let index = match self.verified.get(peer) {
            Some(i) => *i,
            None => return false,
        };

        origin == &self.origins[index]
    }

    pub fn get_address(&self, peer: &PeerId) -> Option<&Address> {
        let index = self.verified.get(peer)?;

        let address = self.addresses.get(*index)?;

        Some(address)
    }

    pub fn get_name(&self, peer: &PeerId) -> Option<&str> {
        let index = self.verified.get(peer)?;

        let name = self.names.get(*index)?;

        Some(name)
    }

    /// Add as verified user.
    pub fn add_peer(
        &mut self,
        peer: PeerId,
        msg_sig: ChatSig,
        addrs: Address,
        name: Option<String>,
    ) {
        if self.verified.contains_key(&peer) {
            return;
        }

        let index = self.peers.len();

        self.peers.push(peer);
        self.origins.push(msg_sig);
        self.addresses.push(addrs);

        if let Some(name) = name {
            self.names.push(name);
        }

        self.verified.insert(peer, index);
    }

    pub fn ban_peer(&mut self, peer: &PeerId) {
        let i = match self.verified.get(peer) {
            Some(i) => *i,
            None => return,
        };

        if i < self.ban_index {
            return;
        }

        if i == self.ban_index {
            self.ban_index += 1;
            return;
        }

        let last = self.ban_index;

        self.peers.swap(i, last);
        self.origins.swap(i, last);
        self.addresses.swap(i, last);
        self.names.swap(i, last);

        //let index = self.verified.get_mut(peer).unwrap();
        //*index = last;

        //let last_peer = self.peers[i];
        //let index = self.verified.get_mut(last_peer).unwrap();
        //*index = i;

        self.verified.entry(*peer).or_insert(last);
        self.verified.entry(self.peers[i]).or_insert(i);

        self.ban_index += 1;
    }
}

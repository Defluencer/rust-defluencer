pub mod beacon;
pub mod blog;
pub mod chat;
pub mod comments;
pub mod feed;
pub mod friends;
pub mod identity;
pub mod live;
pub mod mime_type;
pub mod moderation;
pub mod signature;
pub mod video;

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

use cid::{multibase::Base, multihash::MultihashGeneric, Cid};

/// Ethereum address
pub type Address = [u8; 20];

/// Peer IDs as CIDs v1
// https://github.com/libp2p/specs/blob/master/peer-ids/peer-ids.md#string-representation
pub type PeerId = Cid;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn peer_id_from_str(peer_id: &str) -> Result<PeerId> {
    let decoded = Base::Base58Btc.decode(peer_id)?;

    let multihash = MultihashGeneric::from_bytes(&decoded)?;

    let cid = Cid::new_v1(0x70, multihash);

    Ok(cid)
}

pub type IPNSAddress = Cid;

#[serde_as]
#[derive(
    Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq, Default, Hash, PartialOrd, Ord,
)]
pub struct IPLDLink {
    #[serde(rename = "/")]
    #[serde_as(as = "DisplayFromStr")]
    pub link: Cid,
}

impl From<Cid> for IPLDLink {
    fn from(cid: Cid) -> Self {
        Self { link: cid }
    }
}

/// Compute the Keccak-256 hash of input bytes.
pub fn keccak256(bytes: &[u8]) -> [u8; 32] {
    use tiny_keccak::{Hasher, Keccak};
    let mut output = [0u8; 32];
    let mut hasher = Keccak::v256();
    hasher.update(bytes);
    hasher.finalize(&mut output);
    output
}

use std::fmt::Display;

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

use cid::Cid;

use multibase::Base;

use multihash::MultihashGeneric;
type Multihash = MultihashGeneric<64>;

/// Ethereum address
pub type Address = [u8; 20];

/// Peer IDs as CIDs v1
// https://github.com/libp2p/specs/blob/master/peer-ids/peer-ids.md#peer-ids
#[serde_as]
#[derive(
    Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default, Hash, PartialOrd, Ord,
)]
pub struct PeerId(#[serde_as(as = "DisplayFromStr")] Cid);

impl core::str::FromStr for PeerId {
    type Err = Box<dyn std::error::Error>;

    fn from_str(cid_str: &str) -> Result<Self, Self::Err> {
        Self::try_from(cid_str)
    }
}

impl TryFrom<String> for PeerId {
    type Error = Box<dyn std::error::Error>;

    fn try_from(string: String) -> Result<Self, Self::Error> {
        Self::try_from(string.as_str())
    }
}

impl TryFrom<&str> for PeerId {
    type Error = Box<dyn std::error::Error>;

    fn try_from(str: &str) -> Result<Self, Self::Error> {
        // https://github.com/libp2p/specs/blob/master/peer-ids/peer-ids.md#string-representation
        let decoded = Base::Base58Btc.decode(str)?;

        let multihash = Multihash::from_bytes(&decoded)?;

        let cid = Cid::new_v1(/* Libp2p key */ 0x72, multihash);

        Ok(Self(cid))
    }
}

impl TryFrom<Cid> for PeerId {
    type Error = cid::Error;

    fn try_from(cid: Cid) -> std::result::Result<Self, Self::Error> {
        if cid.codec() != /* libp2p-key */0x72 {
            return Err(cid::Error::ParsingError);
        }

        Ok(PeerId(cid))
    }
}

impl Into<Cid> for PeerId {
    fn into(self) -> Cid {
        self.0
    }
}

impl Display for PeerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_legacy_string())
    }
}

impl PeerId {
    pub fn to_legacy_string(&self) -> String {
        Base::Base58Btc.encode(self.0.hash().to_bytes())
    }

    pub fn to_cid_string(&self) -> String {
        self.0.to_string()
    }
}

/// IPNS address as CIDs v1
#[serde_as]
#[derive(
    Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default, Hash, PartialOrd, Ord,
)]
pub struct IPNSAddress(#[serde_as(as = "DisplayFromStr")] Cid);

impl core::str::FromStr for IPNSAddress {
    type Err = cid::Error;

    fn from_str(cid_str: &str) -> Result<Self, Self::Err> {
        Self::try_from(cid_str)
    }
}

impl TryFrom<String> for IPNSAddress {
    type Error = cid::Error;

    fn try_from(string: String) -> Result<Self, Self::Error> {
        Self::try_from(string.as_str())
    }
}

impl TryFrom<&str> for IPNSAddress {
    type Error = cid::Error;

    fn try_from(str: &str) -> std::result::Result<Self, Self::Error> {
        let cid = Cid::try_from(str)?;

        if cid.codec() != /* libp2p-key */0x72 {
            return Err(cid::Error::ParsingError);
        }

        Ok(IPNSAddress(cid))
    }
}

impl TryFrom<Cid> for IPNSAddress {
    type Error = cid::Error;

    fn try_from(cid: Cid) -> std::result::Result<Self, Self::Error> {
        if cid.codec() != /* libp2p-key */0x72 {
            return Err(cid::Error::ParsingError);
        }

        Ok(IPNSAddress(cid))
    }
}

impl Into<Cid> for IPNSAddress {
    fn into(self) -> Cid {
        self.0
    }
}

impl IPNSAddress {
    pub fn from_pubsub_topic(topic: String) -> Result<Self, Box<dyn std::error::Error>> {
        // https://github.com/ipfs/specs/blob/master/IPNS.md#integration-with-ipfs

        // "/record/".len() == 8
        let decoded = Base::Base64Url.decode(&topic[8..])?;

        // "/ipns/".len() == 6
        let cid = Cid::try_from(&decoded[6..])?;

        let cid_v1 = Cid::new_v1(/* Libp2p key */ 0x72, *cid.hash());

        Ok(Self(cid_v1))
    }

    pub fn to_pubsub_topic(&self) -> String {
        //https://github.com/ipfs/specs/blob/master/IPNS.md#integration-with-ipfs

        let mut bytes = String::from("/ipns/").into_bytes();

        bytes.extend(self.0.hash().to_bytes());

        format!("/record/{}", Base::Base64Url.encode(bytes))
    }
}

impl Display for IPNSAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipns_to_topic() {
        let ipns_base32 = IPNSAddress(
            Cid::try_from("bafzbeiegbnjh5uopd5vc22tgkz6chf7a6ala3x5e47vnhv5sq5bzo46tri").unwrap(),
        );

        let ipns_cidv0 =
            IPNSAddress(Cid::try_from("QmXMuMWm6k3CD3sHV824H2BT1ugcHKF6Tm13ZVM8RhGTB7").unwrap());

        let topic_32 = ipns_base32.to_pubsub_topic();
        let topic_v0 = ipns_cidv0.to_pubsub_topic();

        assert_eq!(topic_32, topic_v0);

        assert_eq!(
            "/record/L2lwbnMvEiCGC1J-0c8fai1qZlZ8I5fg8BYN36Tn6tPXsodDl3PTig",
            topic_32
        );
    }

    #[test]
    fn topic_to_ipns() {
        let record = "/record/L2lwbnMvEiCGC1J-0c8fai1qZlZ8I5fg8BYN36Tn6tPXsodDl3PTig";

        let ipns = IPNSAddress::from_pubsub_topic(record.to_owned()).unwrap();

        assert_eq!(
            ipns.to_string(),
            "bafzbeiegbnjh5uopd5vc22tgkz6chf7a6ala3x5e47vnhv5sq5bzo46tri"
        );
    }
}

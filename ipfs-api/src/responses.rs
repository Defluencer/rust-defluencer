use std::collections::HashMap;

use cid::{
    multibase::{decode, Base},
    multihash::MultihashGeneric,
    Cid,
};

use strum::{self, Display, EnumString};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AddResponse {
    #[serde(rename = "Hash")]
    pub hash: String,
}

impl TryFrom<AddResponse> for Cid {
    type Error = cid::Error;

    fn try_from(response: AddResponse) -> Result<Self, Self::Error> {
        Cid::try_from(response.hash)
    }
}

#[derive(Debug, Deserialize)]
pub struct PubsubSubResponse {
    pub from: String,
    pub data: String,
}

pub struct PubSubMessage {
    /// Peer Id as Cid v1
    pub from: Cid,
    pub data: Vec<u8>,
}

impl TryFrom<PubsubSubResponse> for PubSubMessage {
    type Error = cid::Error;

    fn try_from(response: PubsubSubResponse) -> Result<Self, Self::Error> {
        let PubsubSubResponse { from, data } = response;

        //Use Peer ID as CID v1 instead of multihash btc58 encoded
        // https://github.com/libp2p/specs/blob/master/peer-ids/peer-ids.md#string-representation
        let decoded = Base::Base58Btc.decode(from)?;
        let multihash = MultihashGeneric::from_bytes(&decoded)?;
        let cid = Cid::new_v1(0x70, multihash);

        let (_, data) = decode(data)?;

        Ok(Self { from: cid, data })
    }
}

#[derive(Debug, Deserialize)]
pub struct DagPutResponse {
    #[serde(rename = "Cid")]
    pub cid: CidString,
}

#[derive(Debug, Deserialize)]
pub struct CidString {
    #[serde(rename = "/")]
    pub cid_string: String,
}

impl TryFrom<DagPutResponse> for Cid {
    type Error = cid::Error;

    fn try_from(response: DagPutResponse) -> Result<Self, Self::Error> {
        Cid::try_from(response.cid.cid_string)
    }
}

#[derive(Debug, Deserialize)]
pub struct NamePublishResponse {
    ///IPNS Name
    #[serde(rename = "Name")]
    pub name: String,

    /// CID
    #[serde(rename = "Value")]
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct NameResolveResponse {
    #[serde(rename = "Path")]
    pub path: String,
}

impl TryFrom<NameResolveResponse> for Cid {
    type Error = cid::Error;

    fn try_from(response: NameResolveResponse) -> Result<Self, Self::Error> {
        Cid::try_from(response.path)
    }
}

#[derive(Debug, Deserialize)]
pub struct KeyListResponse {
    #[serde(rename = "Keys")]
    pub keys: Vec<KeyPair>,
}

#[derive(Debug, Deserialize)]
pub struct KeyPair {
    #[serde(rename = "Id")]
    pub id: String,

    #[serde(rename = "Name")]
    pub name: String,
}

pub type KeyList = HashMap<String, Cid>;

impl From<KeyListResponse> for KeyList {
    fn from(response: KeyListResponse) -> Self {
        response
            .keys
            .into_iter()
            .filter_map(|keypair| {
                let KeyPair { id, name } = keypair;

                match Cid::try_from(id) {
                    Ok(cid) => Some((name, cid)),
                    Err(_) => None,
                }
            })
            .collect()
    }
}

#[derive(Debug, Deserialize)]
pub struct IdResponse {
    #[serde(rename = "ID")]
    pub id: String,
}

impl TryFrom<IdResponse> for Cid {
    type Error = cid::Error;

    fn try_from(response: IdResponse) -> Result<Self, Self::Error> {
        let decoded = Base::Base58Btc.decode(response.id)?;
        let multihash = MultihashGeneric::from_bytes(&decoded)?;
        let cid = Cid::new_v1(0x70, multihash);

        Ok(cid)
    }
}

#[derive(Debug, Deserialize)]
pub struct PinAddResponse {
    #[serde(rename = "Pins")]
    pub pins: Vec<String>,

    #[serde(rename = "Progress")]
    pub progress: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PinRmResponse {
    #[serde(rename = "Pins")]
    pub pins: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct PinLsResponse {
    #[serde(rename = "Keys")]
    pub keys: HashMap<String, Pin>,
}

#[derive(Debug, Deserialize)]
pub struct Pin {
    #[serde(rename = "Type")]
    pub mode: PinMode,
}

#[derive(Debug, Display, EnumString, Deserialize)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum PinMode {
    All,
    Recursive,
    Direct,
    Indirect,
}

pub type PinList = HashMap<Cid, PinMode>;

impl From<PinLsResponse> for PinList {
    fn from(response: PinLsResponse) -> Self {
        response
            .keys
            .into_iter()
            .filter_map(|(key, value)| match Cid::try_from(key) {
                Ok(cid) => Some((cid, value.mode)),
                Err(_) => None,
            })
            .collect()
    }
}

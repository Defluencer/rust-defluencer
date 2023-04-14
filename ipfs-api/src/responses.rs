use std::collections::HashMap;

use cid::{multibase::decode, Cid};

use linked_data::types::{IPNSAddress, PeerId};

use strum::{self, Display, EnumString};

use serde::{Deserialize, Serialize};

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
    pub from: PeerId,
    pub data: Vec<u8>,
}

impl TryFrom<PubsubSubResponse> for PubSubMessage {
    type Error = cid::Error;

    fn try_from(response: PubsubSubResponse) -> Result<Self, Self::Error> {
        let PubsubSubResponse { from, data } = response;

        let from = PeerId::try_from(from)?;
        let (_, data) = decode(data)?;

        Ok(Self { from, data })
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
    /// Address
    #[serde(rename = "Id")]
    pub id: String,

    /// Key Name
    #[serde(rename = "Name")]
    pub name: String,
}

pub type KeyList = HashMap<String, IPNSAddress>;

impl From<KeyListResponse> for KeyList {
    fn from(response: KeyListResponse) -> Self {
        response
            .keys
            .into_iter()
            .filter_map(|keypair| {
                let KeyPair { id, name } = keypair;

                match IPNSAddress::try_from(id) {
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

impl TryFrom<IdResponse> for PeerId {
    type Error = cid::Error;

    fn try_from(response: IdResponse) -> Result<Self, Self::Error> {
        PeerId::try_from(response.id)
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

//TODO find a way to stop depending on this!
#[derive(Debug, Display, Clone, Copy, PartialEq, Eq, EnumString, Serialize, Deserialize)]
pub enum Codec {
    #[strum(serialize = "dag-cbor")]
    DagCbor = 0x71,

    #[strum(serialize = "dag-jose")]
    DagJose = 0x85,

    #[strum(serialize = "dag-json")]
    DagJson = 0x0129,
}

impl Default for Codec {
    fn default() -> Self {
        Codec::DagCbor
    }
}

#[derive(Debug, Deserialize)]
pub struct DHTPutResponse {
    #[serde(rename = "Extra")]
    pub extra: Option<String>,

    #[serde(rename = "ID")]
    pub id: Option<String>,

    #[serde(rename = "Responses")]
    pub responses: Vec<Response>,

    #[serde(rename = "Type")]
    pub dht_put_response_type: usize,
}

#[derive(Debug, Deserialize)]
pub struct Response {
    #[serde(rename = "Addrs")]
    pub addrs: Vec<String>,

    #[serde(rename = "ID")]
    pub id: String,
}

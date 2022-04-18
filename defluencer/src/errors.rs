use thiserror::Error;

use crate::indexing::hamt;

#[derive(Error, Debug)]
pub enum Error {
    #[error("ProtoBuf: {0}")]
    ProtoBuf(#[from] prost::DecodeError),

    #[error("HAMT: {0}")]
    HAMT(#[from] hamt::HAMTError),

    #[error("BIP-39: {0}")]
    BIP29(#[from] anyhow::Error),

    #[error("PKCS8: {0}")]
    PKCS8(#[from] pkcs8::Error),

    #[error("Elliptic Curve: {0}")]
    EllipticCurve(#[from] elliptic_curve::Error),

    #[error("Signature: {0}")]
    Signatue(#[from] signature::Error),

    #[cfg(target_arch = "wasm32")]
    #[error("Web3: {0}")]
    Web3(#[from] web3::Error),

    #[error("Serde: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Cid: {0}")]
    Cid(#[from] cid::Error),

    #[error("UTF-8: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("Multibase: {0}")]
    Multibase(#[from] multibase::Error),

    #[error("Ipfs: {0}")]
    IpfsApi(#[from] ipfs_api::errors::Error),

    #[error("IO: {0}")]
    IO(#[from] std::io::Error),

    #[error("Jose: Cannot verify signature")]
    Jose,

    #[error("Defluencer: Could not find")]
    NotFound,

    #[error("Defluencer: Already present")]
    AlreadyAdded,

    #[error("Defluencer: Cannot process image, please use a supported image type")]
    Image,

    #[error("Defluencer: Cannot process file, please use a markdown file")]
    Markdown,
}

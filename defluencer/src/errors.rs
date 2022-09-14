use thiserror::Error;

use crate::indexing::hamt;

#[derive(Error, Debug)]
pub enum Error {
    #[cfg(target_arch = "wasm32")]
    #[error("JS: {0}")]
    JsError(js_sys::JsString),

    #[cfg(not(target_arch = "wasm32"))]
    #[error("Ledger: {0}")]
    Ledger(#[from] ledger_zondax_generic::LedgerAppError<ledger_transport_hid::LedgerHIDError>),

    #[error("HAMT: {0}")]
    HAMT(#[from] hamt::HAMTError),

    #[error("Elliptic Curve: {0}")]
    EllipticCurve(#[from] k256::elliptic_curve::Error),

    #[error("Signature: {0}")]
    Signatue(#[from] k256::ecdsa::signature::Error),

    #[cfg(target_arch = "wasm32")]
    #[error("Web3: {0}")]
    Web3(#[from] web3::Error),

    #[error("Serde: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Cid: {0}")]
    Cid(#[from] cid::Error),

    #[error("UTF-8: {0}")]
    FromUtf8(#[from] std::string::FromUtf8Error),

    #[error("UTF-8: {0}")]
    Utf8(#[from] core::str::Utf8Error),

    #[error("Multibase: {0}")]
    Multibase(#[from] multibase::Error),

    #[error("Multihash: {0}")]
    Multihash(#[from] multihash::Error),

    #[error("Ipfs: {0}")]
    IpfsApi(#[from] ipfs_api::errors::Error),

    #[error("IO: {0}")]
    IO(#[from] std::io::Error),

    #[error("IPNS: {0}")]
    IPNS(#[from] ipns_records::Error),

    #[error("DAG-JOSE: {0}")]
    DagJose(#[from] dag_jose::Error),

    #[error("Defluencer: Could not find")]
    NotFound,

    #[error("Defluencer: Already present")]
    AlreadyAdded,

    #[error("Defluencer: Cannot process image, please use a supported image type")]
    Image,

    #[error("Defluencer: Cannot process file, please use a markdown file")]
    Markdown,

    #[error("IPNS Address Mismatch")]
    IPNSMismatch,
}

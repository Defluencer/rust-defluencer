use std::collections::TryReserveError;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Unknown Value Type")]
    UnknownValueType,

    #[error("Unknown Key Type")]
    UnknownKeyType,

    #[error("Unknown Chunking Strategy")]
    UnknownChunkingStrategy,

    #[error("Unknown Codec")]
    UnknownCodec,

    #[error("Ipld Type Error: {0}")]
    IpldTypeError(#[from] libipld_core::error::TypeError),

    #[error("DAG CBOR Encode: {0}")]
    Encode(#[from] serde_ipld_dagcbor::EncodeError<TryReserveError>),

    #[error("DAG CBOR Decode: {0}")]
    Decode(#[from] serde_ipld_dagcbor::DecodeError<TryReserveError>),

    #[error("Serde: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Multihash: {0}")]
    Multihash(#[from] multihash::Error),

    #[error("Ipfs: {0}")]
    IpfsApi(#[from] ipfs_api::errors::Error),
}

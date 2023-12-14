use std::{convert::Infallible, collections::TryReserveError};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Record signature must not be empty")]
    EmptySignature,

    #[error("Record data must not be empty")]
    EmptyData,

    #[error("Record and address must match")]
    AddressMismatch,

    #[error("Record & document information must match")]
    DataMismatch,

    #[error("Protobuf: {0}")]
    Decode(#[from] prost::DecodeError),

    #[error("Signature: {0}")]
    Signatue(#[from] signature::Error),

    #[error("Encoding: {0}")]
    SPKI(#[from] spki::Error),

    #[error("DAG-CBOR: {0}")]
    DAGCBORDecode(#[from] serde_ipld_dagcbor::DecodeError<Infallible>),

    #[error("DAG-CBOR: {0}")]
    DAGCBOREncode(#[from] serde_ipld_dagcbor::EncodeError<TryReserveError>),
}

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
}

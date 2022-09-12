use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Signature: {0}")]
    Signatue(#[from] signature::Error),

    #[error("Signature: {0}")]
    Decode(#[from] prost::DecodeError),
}

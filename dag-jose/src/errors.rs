use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("DAG-JOSE: No header present")]
    Header,

    #[error("Signature: {0}")]
    Signatue(#[from] signature::Error),

    #[error("Multibase: {0}")]
    Multibase(#[from] multibase::Error),

    #[error("Cid: {0}")]
    Cid(#[from] cid::Error),

    #[error("Serde Json: {0}")]
    SerdeJson(#[from] serde_json::Error),
}

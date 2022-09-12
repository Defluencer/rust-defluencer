use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("JOSE: Unimplemented Cryptography")]
    Crypto,

    #[error("JOSE: No header present")]
    Header,

    #[error("Signature: {0}")]
    Signatue(#[from] signature::Error),
}

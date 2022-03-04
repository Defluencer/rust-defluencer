use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Ipfs: {0}")]
    Ipfs(#[from] ipfs_api::errors::Error),

    #[error("IO: {0}")]
    IO(#[from] std::io::Error),
}

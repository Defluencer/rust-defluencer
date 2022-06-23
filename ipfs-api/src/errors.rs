use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Serde: {0}")]
    Serde(#[from] serde_json::error::Error),

    #[error("UTF-8: {0}")]
    FromUtf8(#[from] std::string::FromUtf8Error),

    #[error("UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),

    #[error("Reqwest: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("Cid: {0}")]
    Cid(#[from] cid::Error),

    #[error("Ipfs: {0}")]
    Ipfs(#[from] IPFSError),

    #[error("Ipns: Key not found")]
    Ipns,

    #[error("Parse: {0}")]
    Parse(#[from] url::ParseError),

    #[error("IO: {0}")]
    IO(#[from] std::io::Error),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IPFSError {
    #[serde(rename = "Message")]
    pub message: String,

    #[serde(rename = "Code")]
    pub code: u64,

    #[serde(rename = "Type")]
    pub error_type: String,
}

impl std::error::Error for IPFSError {}

impl fmt::Display for IPFSError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match serde_json::to_string_pretty(&self) {
            Ok(e) => write!(f, "{}", e),
            Err(e) => write!(f, "{}", e),
        }
    }
}

impl From<IPFSError> for std::io::Error {
    fn from(error: IPFSError) -> Self {
        std::io::Error::new(std::io::ErrorKind::Other, error)
    }
}

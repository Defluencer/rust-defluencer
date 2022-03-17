use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
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

    #[error("Defluencer: Cannot follow user, was already following")]
    Follow,

    #[error("Defluencer: Cannot unfollow user, was not following")]
    UnFollow,

    #[error("Defluencer: Cannot process image, please use a supported image type")]
    Image,

    #[error("Defluencer: Cannot process file, please use a markdown file")]
    Markdown,

    #[error("Defluencer: Content not found")]
    ContentNotFound,

    #[error("Defluencer: Comment not found")]
    CommentNotFound,

    #[error("Defluencer: Content already present")]
    ContentAdded,

    #[error("Defluencer: Comment already present")]
    CommentAdded,
}

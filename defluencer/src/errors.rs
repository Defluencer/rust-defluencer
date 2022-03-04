use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Ipfs: {0}")]
    IpfsApi(#[from] ipfs_api::errors::Error),

    #[error("IO: {0}")]
    IO(#[from] std::io::Error),

    #[error("Defluencer: Cannot follow user, was already following")]
    Follow,

    #[error("Defluencer: Cannot unfollow user, was not following")]
    UnFollow,

    #[error("Defluencer: Cannot process image, please use a supported image type")]
    Image,

    #[error("Defluencer: Cannot process file, please use a markdown file")]
    Markdown,

    #[error("Defluencer: Cannot remove content, content not found")]
    RemoveContent,

    #[error("Defluencer: Cannot remove comment, comment not found")]
    RemoveComment,
}
